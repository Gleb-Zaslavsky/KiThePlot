//! Controller module (MVC "C").
//!
//! Responsibilities:
//! - Accept `Action` values from the view.
//! - Validate and translate actions into `Command`s.
//! - Mutate `PlotModel` and data table state.
//! - Maintain undo/redo stacks.
//! - Handle import/export workflows.
//!
//! Architecture notes:
//! - Actions are user intents; Controller validates and turns them into Commands.
//! - Commands are the single source of truth for mutations, enabling undo/redo.
//! - Export path aims to mirror on-screen layout to avoid visual drift.
//! - Axis titles are positioned relative to the plotting area, not the full canvas, to avoid
//!   vertical drift with font-size changes.

pub mod action;
pub mod command;

pub use action::*;
pub use command::*;

use std::fmt::{Display, Formatter};
use std::path::Path;

use crate::model::{
    AxisConfig, AxisKind, AxesConfig, Color, DataSource, DataTable, LayoutConfig, LegendConfig,
    LegendPosition, LineStyle, MarkerStyle, PlotModel, RangePolicy, ScaleType, SeriesId,
    SeriesModel, SeriesStyle, TickConfig, ImageFormat, ImageSize,
};
use plotters::coord::Shift;
use plotters::coord::types::RangedCoordf32;
use plotters::prelude::*;
use plotters::style::Color as PlottersColor;

/// EN: UI notification severity.
/// RU: Uroven uvedomleniya dlya UI.
#[derive(Clone, Copy)]
pub enum NotificationLevel {
    Info,
    Error,
}

/// EN: Notification shown in the status block of the editor.
/// RU: Soobshchenie v statuse redaktora.
#[derive(Clone)]
pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
}

/// EN: Controller-level error for validation and user-safe feedback.
/// RU: Oshibka kontrollera dlya validatsii i bezopasnoy obratnoy svyazi.
#[derive(Debug)]
pub enum ControllerError {
    EmptyAxisLabel,
    EmptyChartTitle,
    InvalidRange { min: f64, max: f64 },
    InvalidLineWidth(f32),
    SeriesNotFound(SeriesId),
    ColumnNotFound(String),
    DataLoadFailed(String),
    ExportFailed(String),
    NoDataLoaded,
    CannotRemoveLastSeries,
    UnsupportedAction(&'static str),
}

impl Display for ControllerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ControllerError::EmptyAxisLabel => {
                write!(f, "Axis label cannot be empty / Podpis osi ne mozhet byt pustoy")
            }
            ControllerError::EmptyChartTitle => {
                write!(f, "Chart title cannot be empty / Zagolovok ne mozhet byt pustym")
            }
            ControllerError::InvalidRange { min, max } => write!(
                f,
                "Invalid range: min ({min}) must be < max ({max}) / Nekorrektnyy diapazon"
            ),
            ControllerError::InvalidLineWidth(width) => write!(
                f,
                "Invalid line width: {width} / Nekorrektnaya tolshchina linii"
            ),
            ControllerError::SeriesNotFound(id) => {
                write!(f, "Series {:?} not found / Seriya ne naydena", id)
            }
            ControllerError::ColumnNotFound(name) => {
                write!(f, "Column not found: {name} / Stolbets ne naiden")
            }
            ControllerError::DataLoadFailed(err) => {
                write!(f, "Failed to load data: {err} / Ne udalos zagruzit dannye")
            }
            ControllerError::ExportFailed(err) => {
                write!(f, "Failed to export: {err} / Ne udalos sohranit grafik")
            }
            ControllerError::NoDataLoaded => {
                write!(f, "No data loaded / Dannye ne zagruzheny")
            }
            ControllerError::CannotRemoveLastSeries => write!(
                f,
                "Cannot remove last series / Nelyzya udalit poslednyuyu seriyu"
            ),
            ControllerError::UnsupportedAction(name) => write!(
                f,
                "Action is not implemented yet: {name} / Deystvie poka ne realizovano"
            ),
        }
    }
}

impl std::error::Error for ControllerError {}

/// EN: Main MVC controller. Owns model, data table, command history and notifications.
/// RU: Glavnyy MVC-kontroller. Hranit model, tablitsu dannyh, istoriyu komand i uvedomleniya.
pub struct PlotController {
    pub model: PlotModel,
    pub undo_stack: Vec<Command>,
    pub redo_stack: Vec<Command>,
    data_table: Option<DataTable>,
    notification: Option<Notification>,
    next_series_id: u64,
}

impl PlotController {
    /// EN: Empty initial state; data is loaded via File menu.
    /// RU: Pustoe nachalnoe sostoyanie; dannye zagruzhayutsya cherez File menu.
    pub fn new() -> Self {
        Self {
            model: PlotModel::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            data_table: None,
            notification: Some(Notification {
                level: NotificationLevel::Info,
                message: "Ready. Load CSV/TXT from Files menu / Gotovo. Zagruzite CSV/TXT".to_owned(),
            }),
            next_series_id: 2,
        }
    }

    /// Returns latest notification for status banner.
    pub fn notification(&self) -> Option<&Notification> {
        self.notification.as_ref()
    }

    /// Returns names of all loaded numeric columns.
    pub fn available_columns(&self) -> Vec<String> {
        self.data_table
            .as_ref()
            .map(|t| t.column_names())
            .unwrap_or_default()
    }

    /// Indicates whether any dataset has been loaded.
    pub fn has_data(&self) -> bool {
        self.data_table.is_some()
    }

    /// EN: Public API for embedding editor in another crate.
    /// RU: Public API dlya vstraivaniya redaktora v drugoy crate.
    pub fn load_from_data_source(
        &mut self,
        source: &dyn DataSource,
    ) -> Result<(), ControllerError> {
        let table = DataTable::from_data_source(source).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table)
    }

    /// Returns currently selected `(x, y)` points for a series.
    pub fn points_for_series(&self, series_id: SeriesId) -> Result<Vec<(f32, f32)>, ControllerError> {
        let table = self.data_table.as_ref().ok_or(ControllerError::NoDataLoaded)?;
        let series = self.find_series(series_id)?;
        table
            .points_for_columns(&series.x_column, &series.y_column)
            .map_err(ControllerError::DataLoadFailed)
    }

    /// Main action dispatcher invoked from the view each frame.
    pub fn dispatch(&mut self, action: Action) -> Result<(), ControllerError> {
        let result = match action {
            Action::ImportFromCsv { path } => self.import_csv(&path),
            Action::ImportFromTxt { path } => self.import_txt(&path),
            Action::SetChartTitle(new_title) => {
                let trimmed = new_title.trim().to_owned();
                if trimmed.is_empty() {
                    return self.fail(ControllerError::EmptyChartTitle);
                }
                self.execute_command(Command::SetChartTitle {
                    old: self.model.layout.title.clone(),
                    new: trimmed,
                })
            }
            Action::SetAxisLabel { axis, label } => {
                let trimmed = label.trim().to_owned();
                if trimmed.is_empty() {
                    return self.fail(ControllerError::EmptyAxisLabel);
                }
                self.execute_command(Command::SetAxisLabel {
                    axis,
                    old: self.axis(axis).label.clone(),
                    new: trimmed,
                })
            }
            Action::SetAxisLabelFontSize { axis, font_size } => {
                self.execute_command(Command::SetAxisLabelFontSize {
                    axis,
                    old: self.axis(axis).label_font_size,
                    new: font_size.max(8),
                })
            }
            Action::SetAxisTitleFontSize { axis, font_size } => {
                self.execute_command(Command::SetAxisTitleFontSize {
                    axis,
                    old: self.axis(axis).axis_title_font_size,
                    new: font_size.max(8),
                })
            }
            Action::SetAxisScale { axis, scale } => self.execute_command(Command::SetAxisScale {
                axis,
                old: self.axis(axis).scale,
                new: scale,
            }),
            Action::SetAxisRange { axis, range } => {
                if let RangePolicy::Manual { min, max } = range {
                    if min >= max {
                        return self.fail(ControllerError::InvalidRange { min, max });
                    }
                }
                self.execute_command(Command::SetAxisRange {
                    axis,
                    old: self.axis(axis).range.clone(),
                    new: range,
                })
            }
            Action::SetAxisMajorTickStep { axis, step } => {
                self.execute_command(Command::SetAxisMajorTickStep {
                    axis,
                    old: self.axis(axis).ticks.major_step,
                    new: step,
                })
            }
            Action::SetAxisMinorTicks { axis, per_major } => {
                self.execute_command(Command::SetAxisMinorTicks {
                    axis,
                    old: self.axis(axis).ticks.minor_per_major,
                    new: per_major,
                })
            }
            Action::SetLegendVisible(visible) => {
                let mut next = self.model.legend.clone();
                next.visible = visible;
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendTitle(title) => {
                let mut next = self.model.legend.clone();
                next.title = title.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendPosition(position) => {
                let mut next = self.model.legend.clone();
                next.position = position;
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendFontSize(font_size) => {
                let mut next = self.model.legend.clone();
                next.font_size = font_size.max(8);
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendFontColor(color) => {
                let mut next = self.model.legend.clone();
                next.font_color = color;
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLayoutMargin(margin) => {
                let mut next = self.model.layout.clone();
                next.margin = margin;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetXLabelAreaSize(size) => {
                let mut next = self.model.layout.clone();
                next.x_label_area_size = size;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetYLabelAreaSize(size) => {
                let mut next = self.model.layout.clone();
                next.y_label_area_size = size;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetLabelFontSize(size) => {
                let mut next = self.model.layout.clone();
                next.title_font_size = size.max(8);
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetLabelFontColor(color) => {
                let mut next = self.model.layout.clone();
                next.title_font_color = color;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::AddSeries { name, x_column, y_column } => {
                let (default_x, default_y) = self.default_xy_columns()?;
                let x_final = if x_column.is_empty() { default_x } else { x_column };
                let y_final = if y_column.is_empty() { default_y } else { y_column };
                self.ensure_column_exists(&x_final)?;
                self.ensure_column_exists(&y_final)?;

                let series = SeriesModel {
                    id: SeriesId(self.next_series_id),
                    name: if name.trim().is_empty() {
                        format!("Series {}", self.next_series_id)
                    } else {
                        name
                    },
                    x_column: x_final,
                    y_column: y_final,
                    style: SeriesStyle {
                        color: self.color_for_series(self.next_series_id),
                        line_width: 2.0,
                        line_style: LineStyle::Solid,
                        marker: None,
                    },
                    visible: true,
                };
                self.next_series_id += 1;
                self.execute_command(Command::AddSeries {
                    series,
                    index: self.model.series.len(),
                })
            }
            Action::RemoveSeries { series_id } => {
                if self.model.series.len() <= 1 {
                    return self.fail(ControllerError::CannotRemoveLastSeries);
                }
                let Some((index, series)) = self.find_series_index(series_id) else {
                    return self.fail(ControllerError::SeriesNotFound(series_id));
                };
                self.execute_command(Command::RemoveSeries { series, index })
            }
            Action::RenameSeries { series_id, name } => {
                let old = self.find_series(series_id)?.name.clone();
                self.execute_command(Command::RenameSeries {
                    series_id,
                    old,
                    new: name,
                })
            }
            Action::SetSeriesVisibility { series_id, visible } => {
                let old = self.find_series(series_id)?.visible;
                self.execute_command(Command::SetSeriesVisibility {
                    series_id,
                    old,
                    new: visible,
                })
            }
            Action::SetSeriesXColumn { series_id, x_column } => {
                self.ensure_column_exists(&x_column)?;
                let old = self.find_series(series_id)?.x_column.clone();
                self.execute_command(Command::SetSeriesXColumn {
                    series_id,
                    old,
                    new: x_column,
                })
            }
            Action::SetSeriesYColumn { series_id, y_column } => {
                self.ensure_column_exists(&y_column)?;
                let old = self.find_series(series_id)?.y_column.clone();
                self.execute_command(Command::SetSeriesYColumn {
                    series_id,
                    old,
                    new: y_column,
                })
            }
            Action::SetSeriesColor { series_id, color } => {
                let old = self.find_series(series_id)?.style.color;
                self.execute_command(Command::SetSeriesColor {
                    series_id,
                    old,
                    new: color,
                })
            }
            Action::SetSeriesLineWidth { series_id, width } => {
                if width <= 0.0 {
                    return self.fail(ControllerError::InvalidLineWidth(width));
                }
                let old = self.find_series(series_id)?.style.line_width;
                self.execute_command(Command::SetSeriesLineWidth {
                    series_id,
                    old,
                    new: width,
                })
            }
            Action::SetSeriesLineStyle { series_id, line_style } => {
                let old = self.find_series(series_id)?.style.line_style;
                self.execute_command(Command::SetSeriesLineStyle {
                    series_id,
                    old,
                    new: line_style,
                })
            }
            Action::SetSeriesMarker { series_id, marker, size } => {
                let old = self.find_series(series_id)?.style.marker.clone();
                let new = marker.map(|shape| MarkerStyle { shape, size });
                self.execute_command(Command::SetSeriesMarker {
                    series_id,
                    old,
                    new,
                })
            }
            Action::RequestSaveAs { path, format, size } => self.export_plot(&path, format, size),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            other => self.fail(ControllerError::UnsupportedAction(match other {
                Action::ResetPlot => "ResetPlot",
                _ => "UnknownAction",
            })),
        };

        if result.is_ok() {
            self.notification = Some(Notification {
                level: NotificationLevel::Info,
                message: "Updated / Obnovleno".to_owned(),
            });
        }

        result
    }

    fn import_csv(&mut self, path: &str) -> Result<(), ControllerError> {
        let table = DataTable::from_csv_path(Path::new(path)).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table)
    }

    fn import_txt(&mut self, path: &str) -> Result<(), ControllerError> {
        let table = DataTable::from_txt_path(Path::new(path)).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table)
    }

    fn set_table(&mut self, table: DataTable) -> Result<(), ControllerError> {
        let names = table.column_names();
        if names.len() < 2 {
            return self.fail(ControllerError::DataLoadFailed(
                "Need at least two numeric columns".to_owned(),
            ));
        }
        self.data_table = Some(table);
        let x = names[0].clone();
        let y = names[1].clone();

        if self.model.series.is_empty() {
            self.model.series.push(SeriesModel {
                id: SeriesId(1),
                name: "Series 1".to_owned(),
                x_column: x,
                y_column: y,
                style: SeriesStyle {
                    color: Color {
                        r: 220,
                        g: 50,
                        b: 47,
                        a: 255,
                    },
                    line_width: 2.0,
                    line_style: LineStyle::Solid,
                    marker: None,
                },
                visible: true,
            });
        } else {
            self.model.series[0].x_column = x;
            self.model.series[0].y_column = y;
        }

        self.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Data imported / Dannye zagruzheny".to_owned(),
        });
        Ok(())
    }

    fn default_xy_columns(&self) -> Result<(String, String), ControllerError> {
        let table = self.data_table.as_ref().ok_or(ControllerError::NoDataLoaded)?;
        let names = table.column_names();
        if names.len() < 2 {
            return Err(ControllerError::DataLoadFailed(
                "Need at least two columns".to_owned(),
            ));
        }
        Ok((names[0].clone(), names[1].clone()))
    }

    fn ensure_column_exists(&self, name: &str) -> Result<(), ControllerError> {
        let table = self.data_table.as_ref().ok_or(ControllerError::NoDataLoaded)?;
        if table.has_column(name) {
            Ok(())
        } else {
            Err(ControllerError::ColumnNotFound(name.to_owned()))
        }
    }

    fn export_plot(
        &self,
        path: &str,
        format: ImageFormat,
        size: ImageSize,
    ) -> Result<(), ControllerError> {
        match format {
            ImageFormat::Png => {
                let backend = BitMapBackend::new(path, (size.width, size.height));
                let root = backend.into_drawing_area();
                self.draw_export_chart(&root)?;
                root.present()
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
            }
            ImageFormat::Svg => {
                let backend = SVGBackend::new(path, (size.width, size.height));
                let root = backend.into_drawing_area();
                self.draw_export_chart(&root)?;
                root.present()
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn draw_export_chart<DB: DrawingBackend>(
        &self,
        root: &DrawingArea<DB, Shift>,
    ) -> Result<(), ControllerError> {
        root.fill(&WHITE)
            .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

        if !self.has_data() {
            return Err(ControllerError::NoDataLoaded);
        }

        let mut rendered = Vec::new();
        for series in &self.model.series {
            if !series.visible {
                continue;
            }
            let points = self.points_for_series(series.id)?;
            let scaled = points
                .iter()
                .copied()
                .filter_map(|(x, y)| apply_scale(x, y, self.model.axes.x.scale, self.model.axes.y.scale))
                .collect::<Vec<_>>();
            rendered.push((series, scaled));
        }

        let x_range = resolve_range(&self.model.axes.x.range, &rendered, true, -1.0..1.0);
        let y_range = resolve_range(&self.model.axes.y.range, &rendered, false, -1.0..1.0);

        let effective_x_label_area = self
            .model
            .layout
            .x_label_area_size
            .max(self.model.axes.x.label_font_size + 18)
            .max(self.model.axes.x.axis_title_font_size + 20);
        // Use conservative y-label area to avoid overlap between Y tick labels and Y-axis title
        let effective_y_label_area = self
            .model
            .layout
            .y_label_area_size
            .max(((self.model.axes.y.label_font_size as f32 * 1.6) as u32) + 16)
            .max(self.model.axes.y.axis_title_font_size + 28)
            .max(self.model.axes.y.label_font_size + self.model.axes.y.axis_title_font_size + 28);

        let mut chart = ChartBuilder::on(root)
            .caption(
                self.model.layout.title.clone(),
                ("sans-serif", self.model.layout.title_font_size)
                    .into_font()
                    .color(&RGBColor(
                        self.model.layout.title_font_color.r,
                        self.model.layout.title_font_color.g,
                        self.model.layout.title_font_color.b,
                    )),
            )
            .margin(self.model.layout.margin)
            .x_label_area_size(effective_x_label_area)
            .y_label_area_size(effective_y_label_area)
            .build_cartesian_2d(x_range.clone(), y_range.clone())
            .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

        configure_mesh(
            &mut chart,
            self.model.axes.x.label_font_size,
            self.model.axes.y.label_font_size,
            &self.model.axes.x.ticks,
            &self.model.axes.y.ticks,
            x_range,
            y_range,
        )?;
        draw_axis_titles(
            root,
            &format!(
                "{}{}",
                self.model.axes.x.label,
                scale_suffix(self.model.axes.x.scale)
            ),
            &format!(
                "{}{}",
                self.model.axes.y.label,
                scale_suffix(self.model.axes.y.scale)
            ),
            self.model.axes.x.axis_title_font_size,
            self.model.axes.y.axis_title_font_size,
            effective_x_label_area,
            effective_y_label_area,
            self.model.layout.title_font_size,
            self.model.layout.margin,
        )?;

        for (series, points) in &rendered {
            if points.is_empty() {
                continue;
            }
            let color = RGBColor(series.style.color.r, series.style.color.g, series.style.color.b);
            let style = ShapeStyle::from(&color).stroke_width(series.style.line_width.max(1.0) as u32);

            if series.style.line_style == LineStyle::Dotted {
                chart
                    .draw_series(points.iter().map(|(x, y)| Circle::new((*x, *y), 2, style.filled())))
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
            } else {
                let drawn = chart
                    .draw_series(LineSeries::new(points.iter().copied(), style))
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
                if self.model.legend.visible {
                    drawn
                        .label(series.name.clone())
                        .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
                }
            }
        }

        if self.model.legend.visible {
            chart
                .configure_series_labels()
                .label_font(
                    ("sans-serif", self.model.legend.font_size)
                        .into_font()
                        .color(&RGBColor(
                            self.model.legend.font_color.r,
                            self.model.legend.font_color.g,
                            self.model.legend.font_color.b,
                        )),
                )
                .position(series_label_position(self.model.legend.position))
                .background_style(WHITE.mix(0.8))
                .border_style(BLACK)
                .draw()
                .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
        }

        Ok(())
    }

    fn undo(&mut self) -> Result<(), ControllerError> {
        if let Some(cmd) = self.undo_stack.pop() {
            self.apply_inverse_command(&cmd);
            self.redo_stack.push(cmd);
            Ok(())
        } else {
            self.notification = Some(Notification {
                level: NotificationLevel::Info,
                message: "Nothing to undo / Nechego otmenyat".to_owned(),
            });
            Ok(())
        }
    }

    fn redo(&mut self) -> Result<(), ControllerError> {
        if let Some(cmd) = self.redo_stack.pop() {
            self.apply_command(&cmd);
            self.undo_stack.push(cmd);
            Ok(())
        } else {
            self.notification = Some(Notification {
                level: NotificationLevel::Info,
                message: "Nothing to redo / Nechego povtoryat".to_owned(),
            });
            Ok(())
        }
    }

    fn fail<T>(&mut self, error: ControllerError) -> Result<T, ControllerError> {
        self.notification = Some(Notification {
            level: NotificationLevel::Error,
            message: error.to_string(),
        });
        Err(error)
    }

    fn execute_command(&mut self, command: Command) -> Result<(), ControllerError> {
        self.apply_command(&command);
        self.undo_stack.push(command);
        self.redo_stack.clear();
        Ok(())
    }

    fn apply_command(&mut self, command: &Command) {
        match command {
            Command::SetChartTitle { new, .. } => self.model.layout.title = new.clone(),
            Command::SetAxisLabel { axis, new, .. } => self.axis_mut(*axis).label = new.clone(),
            Command::SetAxisLabelFontSize { axis, new, .. } => {
                self.axis_mut(*axis).label_font_size = *new
            }
            Command::SetAxisTitleFontSize { axis, new, .. } => {
                self.axis_mut(*axis).axis_title_font_size = *new
            }
            Command::SetAxisScale { axis, new, .. } => self.axis_mut(*axis).scale = *new,
            Command::SetAxisRange { axis, new, .. } => self.axis_mut(*axis).range = new.clone(),
            Command::SetAxisMajorTickStep { axis, new, .. } => {
                self.axis_mut(*axis).ticks.major_step = *new
            }
            Command::SetAxisMinorTicks { axis, new, .. } => {
                self.axis_mut(*axis).ticks.minor_per_major = *new
            }
            Command::ReplaceAxisConfig { axis, new, .. } => *self.axis_mut(*axis) = new.clone(),
            Command::AddSeries { series, index } => self.model.series.insert(*index, series.clone()),
            Command::RemoveSeries { index, .. } => {
                self.model.series.remove(*index);
            }
            Command::RenameSeries { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.name = new.clone();
                }
            }
            Command::SetSeriesVisibility { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.visible = *new;
                }
            }
            Command::SetSeriesXColumn { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.x_column = new.clone();
                }
            }
            Command::SetSeriesYColumn { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.y_column = new.clone();
                }
            }
            Command::SetSeriesLineWidth { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_width = *new;
                }
            }
            Command::SetSeriesLineStyle { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_style = *new;
                }
            }
            Command::SetSeriesColor { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.color = *new;
                }
            }
            Command::SetSeriesMarker { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.marker = new.clone();
                }
            }
            Command::ReplaceLegend { new, .. } => self.model.legend = new.clone(),
            Command::ReplaceLayout { new, .. } => self.model.layout = new.clone(),
            Command::Batch { commands } => {
                for c in commands {
                    self.apply_command(c);
                }
            }
        }
    }

    fn apply_inverse_command(&mut self, command: &Command) {
        match command {
            Command::SetChartTitle { old, .. } => self.model.layout.title = old.clone(),
            Command::SetAxisLabel { axis, old, .. } => self.axis_mut(*axis).label = old.clone(),
            Command::SetAxisLabelFontSize { axis, old, .. } => {
                self.axis_mut(*axis).label_font_size = *old
            }
            Command::SetAxisTitleFontSize { axis, old, .. } => {
                self.axis_mut(*axis).axis_title_font_size = *old
            }
            Command::SetAxisScale { axis, old, .. } => self.axis_mut(*axis).scale = *old,
            Command::SetAxisRange { axis, old, .. } => self.axis_mut(*axis).range = old.clone(),
            Command::SetAxisMajorTickStep { axis, old, .. } => self.axis_mut(*axis).ticks.major_step = *old,
            Command::SetAxisMinorTicks { axis, old, .. } => self.axis_mut(*axis).ticks.minor_per_major = *old,
            Command::ReplaceAxisConfig { axis, old, .. } => *self.axis_mut(*axis) = old.clone(),
            Command::AddSeries { index, .. } => {
                self.model.series.remove(*index);
            }
            Command::RemoveSeries { series, index } => self.model.series.insert(*index, series.clone()),
            Command::RenameSeries { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.name = old.clone();
                }
            }
            Command::SetSeriesVisibility { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.visible = *old;
                }
            }
            Command::SetSeriesXColumn { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.x_column = old.clone();
                }
            }
            Command::SetSeriesYColumn { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.y_column = old.clone();
                }
            }
            Command::SetSeriesLineWidth { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_width = *old;
                }
            }
            Command::SetSeriesLineStyle { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_style = *old;
                }
            }
            Command::SetSeriesColor { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.color = *old;
                }
            }
            Command::SetSeriesMarker { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.marker = old.clone();
                }
            }
            Command::ReplaceLegend { old, .. } => self.model.legend = old.clone(),
            Command::ReplaceLayout { old, .. } => self.model.layout = old.clone(),
            Command::Batch { commands } => {
                for c in commands.iter().rev() {
                    self.apply_inverse_command(c);
                }
            }
        }
    }

    fn axis(&self, axis: AxisKind) -> &AxisConfig {
        match axis {
            AxisKind::X => &self.model.axes.x,
            AxisKind::Y => &self.model.axes.y,
        }
    }

    fn axis_mut(&mut self, axis: AxisKind) -> &mut AxisConfig {
        match axis {
            AxisKind::X => &mut self.model.axes.x,
            AxisKind::Y => &mut self.model.axes.y,
        }
    }

    fn find_series(&self, series_id: SeriesId) -> Result<&SeriesModel, ControllerError> {
        self.model
            .series
            .iter()
            .find(|s| s.id == series_id)
            .ok_or(ControllerError::SeriesNotFound(series_id))
    }

    fn find_series_mut_raw(&mut self, series_id: SeriesId) -> Option<&mut SeriesModel> {
        self.model.series.iter_mut().find(|s| s.id == series_id)
    }

    fn find_series_index(&self, series_id: SeriesId) -> Option<(usize, SeriesModel)> {
        self.model
            .series
            .iter()
            .enumerate()
            .find(|(_, s)| s.id == series_id)
            .map(|(idx, s)| (idx, s.clone()))
    }

    fn color_for_series(&self, n: u64) -> Color {
        match n % 6 {
            0 => Color { r: 220, g: 50, b: 47, a: 255 },
            1 => Color { r: 38, g: 139, b: 210, a: 255 },
            2 => Color { r: 133, g: 153, b: 0, a: 255 },
            3 => Color { r: 203, g: 75, b: 22, a: 255 },
            4 => Color { r: 42, g: 161, b: 152, a: 255 },
            _ => Color { r: 108, g: 113, b: 196, a: 255 },
        }
    }
}

impl Default for PlotModel {
    fn default() -> Self {
        Self {
            axes: AxesConfig {
                x: AxisConfig {
                    label: "X".to_owned(),
                    axis_title_font_size: 18,
                    label_font_size: 16,
                    scale: ScaleType::Linear,
                    range: RangePolicy::Auto,
                    ticks: TickConfig {
                        major_step: None,
                        minor_per_major: 4,
                    },
                },
                y: AxisConfig {
                    label: "Y".to_owned(),
                    axis_title_font_size: 18,
                    label_font_size: 16,
                    scale: ScaleType::Linear,
                    range: RangePolicy::Auto,
                    ticks: TickConfig {
                        major_step: None,
                        minor_per_major: 4,
                    },
                },
            },
            series: vec![SeriesModel {
                id: SeriesId(1),
                name: "Series 1".to_owned(),
                x_column: String::new(),
                y_column: String::new(),
                style: SeriesStyle {
                    color: Color {
                        r: 220,
                        g: 50,
                        b: 47,
                        a: 255,
                    },
                    line_width: 2.0,
                    line_style: LineStyle::Solid,
                    marker: None,
                },
                visible: true,
            }],
            legend: LegendConfig {
                visible: true,
                title: Some("Legend".to_owned()),
                position: LegendPosition::TopRight,
                font_size: 16,
                font_color: Color {
                    r: 20,
                    g: 20,
                    b: 20,
                    a: 255,
                },
            },
            layout: LayoutConfig {
                title: "Plot".to_owned(),
                x_label_area_size: 35,
                y_label_area_size: 35,
                margin: 8,
                title_font_size: 24,
                title_font_color: Color {
                    r: 20,
                    g: 20,
                    b: 20,
                    a: 255,
                },
            },
        }
    }
}

fn resolve_range(
    policy: &RangePolicy,
    data: &[(&SeriesModel, Vec<(f32, f32)>)],
    is_x: bool,
    fallback: std::ops::Range<f32>,
) -> std::ops::Range<f32> {
    match policy {
        RangePolicy::Manual { min, max } => (*min as f32)..(*max as f32),
        RangePolicy::Auto => {
            let mut min_v = f32::INFINITY;
            let mut max_v = f32::NEG_INFINITY;
            for (_, points) in data {
                for (x, y) in points {
                    let v = if is_x { *x } else { *y };
                    min_v = min_v.min(v);
                    max_v = max_v.max(v);
                }
            }
            if !min_v.is_finite() || !max_v.is_finite() || min_v >= max_v {
                return fallback;
            }
            let pad = ((max_v - min_v) * 0.05).max(0.1);
            (min_v - pad)..(max_v + pad)
        }
    }
}

fn apply_scale(x: f32, y: f32, x_scale: ScaleType, y_scale: ScaleType) -> Option<(f32, f32)> {
    let sx = match x_scale {
        ScaleType::Linear => Some(x),
        ScaleType::Log10 => (x > 0.0).then(|| x.log10()),
        ScaleType::LogE => (x > 0.0).then(|| x.ln()),
    }?;
    let sy = match y_scale {
        ScaleType::Linear => Some(y),
        ScaleType::Log10 => (y > 0.0).then(|| y.log10()),
        ScaleType::LogE => (y > 0.0).then(|| y.ln()),
    }?;
    Some((sx, sy))
}

fn configure_mesh<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordf32, RangedCoordf32>>,
    x_label_font_size: u32,
    y_label_font_size: u32,
    x_ticks: &TickConfig,
    y_ticks: &TickConfig,
    x_range: std::ops::Range<f32>,
    y_range: std::ops::Range<f32>,
) -> Result<(), ControllerError> {
    let x_labels = labels_from_step(x_range, x_ticks.major_step).unwrap_or(10);
    let y_labels = labels_from_step(y_range, y_ticks.major_step).unwrap_or(10);

    chart
        .configure_mesh()
        .x_desc("")
        .y_desc("")
        .x_label_style(("sans-serif", x_label_font_size))
        .y_label_style(("sans-serif", y_label_font_size))
        .x_labels(x_labels)
        .y_labels(y_labels)
        .max_light_lines(x_ticks.minor_per_major.max(y_ticks.minor_per_major) as usize)
        .draw()
        .map_err(|e| ControllerError::ExportFailed(e.to_string()))
}

fn draw_axis_titles<DB: DrawingBackend>(
    area: &DrawingArea<DB, Shift>,
    x_label: &str,
    y_label: &str,
    x_font_size: u32,
    y_font_size: u32,
    x_label_area: u32,
    y_label_area: u32,
    title_font_size: u32,
    margin: u32,
) -> Result<(), ControllerError> {
    // Anchor axis titles to the plotting area to avoid vertical drift with changing fonts
    let (w, h) = area.dim_in_pixel();
    let x_style = ("sans-serif", x_font_size.max(8)).into_font().color(&BLACK);
    let y_style = ("sans-serif", y_font_size.max(8))
        .into_font()
        .transform(plotters::style::FontTransform::Rotate270)
        .color(&BLACK);

    let cap_h = (title_font_size as i32 + 10).max(12);
    let m = margin as i32;
    let top_y = cap_h + m;
    let bottom_y = h as i32 - (x_label_area as i32) - m;
    let plot_center_y = (top_y + bottom_y) / 2;

    area.draw(&Text::new(
        x_label.to_owned(),
        (w as i32 / 2, h as i32 - (x_label_area as i32 / 2).max(8)),
        x_style,
    ))
    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

    // Place Y title farther left to avoid touching tick numbers: use 2/3 of label area
    let y_x = ((y_label_area as i32 * 2) / 3).max(12);
    area.draw(&Text::new(
        y_label.to_owned(),
        (y_x, plot_center_y),
        y_style,
    ))
    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

    Ok(())
}

fn labels_from_step(range: std::ops::Range<f32>, step: Option<f64>) -> Option<usize> {
    let step = step?;
    if step <= 0.0 {
        return None;
    }
    let span = (range.end - range.start).abs() as f64;
    if span <= 0.0 {
        return None;
    }
    Some(((span / step).round() as usize + 1).clamp(2, 100))
}

fn scale_suffix(scale: ScaleType) -> &'static str {
    match scale {
        ScaleType::Linear => "",
        ScaleType::Log10 => " [log10]",
        ScaleType::LogE => " [ln]",
    }
}

fn series_label_position(value: LegendPosition) -> SeriesLabelPosition {
    match value {
        LegendPosition::TopLeft => SeriesLabelPosition::UpperLeft,
        LegendPosition::TopRight => SeriesLabelPosition::UpperRight,
        LegendPosition::BottomLeft => SeriesLabelPosition::LowerLeft,
        LegendPosition::BottomRight => SeriesLabelPosition::LowerRight,
    }
}
