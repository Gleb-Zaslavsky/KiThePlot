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
    AxesConfig, AxisConfig, AxisKind, Color, DataSource, DataTable, ErrorBarSide, ImageFormat,
    ImageSize, ImportOptions, ImportPreview, LayoutConfig, LegendConfig, LegendPosition, LineStyle,
    MarkerStyle, PlotModel, PublicationDimensions, RangePolicy, ScaleType, SeriesId, SeriesModel,
    SeriesStyle, TickConfig,
};
use crate::view::series::{
    ChartRenderConfig, ErrorBar, ErrorBarSet, LegendItem, SeriesRender, chart_margins_for_legend,
    draw_chart_content, draw_custom_legend, resolve_data_range, resolve_label_areas,
};
use plotters::coord::Shift;
use plotters::prelude::*;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSummary {
    pub source: String,
    pub rows: usize,
    pub columns: usize,
}

#[derive(Clone)]
struct PendingImport {
    path: String,
    options: ImportOptions,
    preview: ImportPreview,
}

/// EN: Controller-level error for validation and user-safe feedback.
/// RU: Oshibka kontrollera dlya validatsii i bezopasnoy obratnoy svyazi.
#[derive(Debug)]
pub enum ControllerError {
    EmptyAxisLabel,
    InvalidRange { min: f64, max: f64 },
    InvalidLineWidth(f32),
    InvalidAreaBaseline(f64),
    SeriesNotFound(SeriesId),
    ColumnNotFound(String),
    DataLoadFailed(String),
    ProjectFailed(String),
    ExportFailed(String),
    NoDataLoaded,
    CannotRemoveLastSeries,
    UnsupportedAction(&'static str),
}

impl Display for ControllerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ControllerError::EmptyAxisLabel => {
                write!(
                    f,
                    "Axis label cannot be empty / Podpis osi ne mozhet byt pustoy"
                )
            }
            ControllerError::InvalidRange { min, max } => write!(
                f,
                "Invalid range: min ({min}) must be < max ({max}) / Nekorrektnyy diapazon"
            ),
            ControllerError::InvalidLineWidth(width) => write!(
                f,
                "Invalid line width: {width} / Nekorrektnaya tolshchina linii"
            ),
            ControllerError::InvalidAreaBaseline(value) => {
                write!(f, "Invalid area baseline: {value}")
            }
            ControllerError::SeriesNotFound(id) => {
                write!(f, "Series {:?} not found / Seriya ne naydena", id)
            }
            ControllerError::ColumnNotFound(name) => {
                write!(f, "Column not found: {name} / Stolbets ne naiden")
            }
            ControllerError::DataLoadFailed(err) => {
                write!(f, "Failed to load data: {err} / Ne udalos zagruzit dannye")
            }
            ControllerError::ProjectFailed(err) => {
                write!(f, "Project error: {err} / Oshibka proekta")
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
    data_source_name: Option<String>,
    project_publication_dimensions: Option<PublicationDimensions>,
    notification: Option<Notification>,
    pending_import: Option<PendingImport>,
    next_series_id: u64,
    data_revision: u64,
    coalesce_axes_ranges: bool,
    continuous_edit_baseline: Option<PlotModel>,
}

impl PlotController {
    /// EN: Empty initial state; data is loaded via File menu.
    /// RU: Pustoe nachalnoe sostoyanie; dannye zagruzhayutsya cherez File menu.
    pub fn new() -> Self {
        Self::with_model(PlotModel::default())
    }

    /// Creates a controller with a host-provided plot model as its initial style/template.
    /// Data can then be supplied with `load_from_data_source`.
    pub fn with_model(model: PlotModel) -> Self {
        let next_series_id = model
            .series
            .iter()
            .map(|series| series.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        Self {
            model,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            data_table: None,
            data_source_name: None,
            project_publication_dimensions: None,
            notification: Some(Notification {
                level: NotificationLevel::Info,
                message: "Ready. Load CSV/TXT from Files menu / Gotovo. Zagruzite CSV/TXT"
                    .to_owned(),
            }),
            pending_import: None,
            next_series_id,
            data_revision: 0,
            coalesce_axes_ranges: false,
            continuous_edit_baseline: None,
        }
    }

    /// Returns latest notification for status banner.
    pub fn notification(&self) -> Option<&Notification> {
        self.notification.as_ref()
    }

    pub fn import_preview(&self) -> Option<(&str, ImportOptions, &ImportPreview)> {
        self.pending_import
            .as_ref()
            .map(|pending| (pending.path.as_str(), pending.options, &pending.preview))
    }

    /// Returns names of all loaded numeric columns.
    pub fn available_columns(&self) -> Vec<String> {
        self.data_table
            .as_ref()
            .map(|t| t.column_names())
            .unwrap_or_default()
    }

    /// Suggests padded bounds from visible data, excluding non-positive values on log axes.
    pub fn suggested_axis_range(&self, axis: AxisKind) -> Option<(f64, f64)> {
        self.data_table.as_ref()?;
        let is_x = axis == AxisKind::X;
        let scale = self.axis(axis).scale;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for series in self.model.series.iter().filter(|series| {
            series.visible
                && (self.model.panels.is_empty()
                    || self
                        .model
                        .panels
                        .iter()
                        .any(|panel| panel.series_ids.contains(&series.id)))
        }) {
            for (x, y) in self.points_for_series(series.id).ok()? {
                let value = if is_x { x } else { y };
                if let Some(value) = scale_value(value, scale) {
                    let value = value_from_scale(value, scale)?;
                    min = min.min(value);
                    max = max.max(value);
                }
            }
            if let Ok(errors) = self.error_bars_for_series(series.id) {
                for (x, y) in errors.bars.iter().flat_map(ErrorBar::range_points) {
                    let value = if is_x { x } else { y };
                    if let Some(value) = value_from_scale(value, scale) {
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
            if !is_x
                && series.style.area_fill_visible
                && let Some(baseline) = series.style.area_baseline
                && let Some(baseline) = scale_value(baseline, scale)
                && let Some(baseline) = value_from_scale(baseline, scale)
            {
                min = min.min(baseline);
                max = max.max(baseline);
            }
        }
        if !min.is_finite() || !max.is_finite() {
            return None;
        }
        let scaled_min = scale_value(min, scale)?;
        let scaled_max = scale_value(max, scale)?;
        let span = scaled_max - scaled_min;
        let padding = if span > 0.0 { span * 0.05 } else { 0.05 };
        let lower = value_from_scale(scaled_min - padding, scale)?;
        let upper = value_from_scale(scaled_max + padding, scale)?;
        if lower.is_finite() && upper.is_finite() && lower < upper {
            Some((lower, upper))
        } else {
            None
        }
    }

    /// Indicates whether any dataset has been loaded.
    pub fn has_data(&self) -> bool {
        self.data_table.is_some()
    }

    /// Monotonically changes whenever the active table is replaced.
    pub fn data_revision(&self) -> u64 {
        self.data_revision
    }

    pub fn data_summary(&self) -> Option<DataSummary> {
        let table = self.data_table.as_ref()?;
        Some(DataSummary {
            source: self
                .data_source_name
                .clone()
                .unwrap_or_else(|| "Data table".to_owned()),
            rows: table.row_count,
            columns: table.columns.len(),
        })
    }

    pub fn project_publication_dimensions(&self) -> Option<PublicationDimensions> {
        self.project_publication_dimensions
    }

    /// EN: Public API for embedding editor in another crate.
    /// RU: Public API dlya vstraivaniya redaktora v drugoy crate.
    pub fn load_from_data_source(
        &mut self,
        source: &dyn DataSource,
    ) -> Result<(), ControllerError> {
        let table = DataTable::from_data_source(source).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table, "Host-provided data".to_owned())
    }

    /// Returns currently selected `(x, y)` points for a series.
    pub fn points_for_series(
        &self,
        series_id: SeriesId,
    ) -> Result<Vec<(f64, f64)>, ControllerError> {
        let table = self
            .data_table
            .as_ref()
            .ok_or(ControllerError::NoDataLoaded)?;
        let series = self.find_series(series_id)?;
        table
            .points_for_columns(&series.x_column, &series.y_column)
            .map_err(ControllerError::DataLoadFailed)
    }

    pub(crate) fn error_bars_for_series(
        &self,
        series_id: SeriesId,
    ) -> Result<ErrorBarSet, ControllerError> {
        let table = self
            .data_table
            .as_ref()
            .ok_or(ControllerError::NoDataLoaded)?;
        let series = self.find_series(series_id)?;
        let Some(x_values) = table.column_values(&series.x_column) else {
            return Err(ControllerError::DataLoadFailed(format!(
                "X column not found: {}",
                series.x_column
            )));
        };
        let Some(y_values) = table.column_values(&series.y_column) else {
            return Err(ControllerError::DataLoadFailed(format!(
                "Y column not found: {}",
                series.y_column
            )));
        };
        let x_symmetric = series
            .x_error_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let y_symmetric = series
            .y_error_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let x_lower = series
            .x_error_lower_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let x_upper = series
            .x_error_upper_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let y_lower = series
            .y_error_lower_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let y_upper = series
            .y_error_upper_column
            .as_deref()
            .and_then(|name| table.column_values(name));
        let x_asymmetric = x_lower.is_some() || x_upper.is_some();
        let y_asymmetric = y_lower.is_some() || y_upper.is_some();
        for (active, name) in [
            (x_asymmetric, series.x_error_lower_column.as_deref()),
            (x_asymmetric, series.x_error_upper_column.as_deref()),
            (!x_asymmetric, series.x_error_column.as_deref()),
            (y_asymmetric, series.y_error_lower_column.as_deref()),
            (y_asymmetric, series.y_error_upper_column.as_deref()),
            (!y_asymmetric, series.y_error_column.as_deref()),
        ] {
            if active && let Some(name) = name {
                validate_error_column_values_in_table(table, name)?;
            }
        }
        if x_symmetric.is_none()
            && y_symmetric.is_none()
            && x_lower.is_none()
            && x_upper.is_none()
            && y_lower.is_none()
            && y_upper.is_none()
        {
            return Ok(ErrorBarSet::default());
        }

        let mut result = ErrorBarSet {
            bars: Vec::with_capacity(x_values.len().min(y_values.len())),
            ..ErrorBarSet::default()
        };
        for index in 0..x_values.len().min(y_values.len()) {
            let (x, y) = (x_values[index], y_values[index]);
            let Some(center) = apply_scale(x, y, self.model.axes.x.scale, self.model.axes.y.scale)
            else {
                continue;
            };
            if self.model.axes.x.scale != ScaleType::Linear
                && interval_has_non_positive_endpoint(x, index, x_symmetric, x_lower, x_upper)
            {
                result.omitted_log_x += 1;
            }
            if self.model.axes.y.scale != ScaleType::Linear
                && interval_has_non_positive_endpoint(y, index, y_symmetric, y_lower, y_upper)
            {
                result.omitted_log_y += 1;
            }
            let x_bounds = error_bounds_for_point(
                x,
                index,
                x_symmetric,
                x_lower,
                x_upper,
                self.model.axes.x.scale,
            );
            let y_bounds = error_bounds_for_point(
                y,
                index,
                y_symmetric,
                y_lower,
                y_upper,
                self.model.axes.y.scale,
            );
            if x_bounds.is_some() || y_bounds.is_some() {
                result.bars.push(ErrorBar {
                    center,
                    x_bounds,
                    y_bounds,
                });
            }
        }
        Ok(result)
    }

    /// Main action dispatcher invoked from the view each frame.
    pub fn dispatch(&mut self, action: Action) -> Result<(), ControllerError> {
        let result = self.dispatch_inner(action);
        if let Err(error) = &result {
            self.notification = Some(Notification {
                level: NotificationLevel::Error,
                message: error.to_string(),
            });
        }
        result
    }

    fn dispatch_inner(&mut self, action: Action) -> Result<(), ControllerError> {
        let result = match action {
            Action::ImportFromCsv { path } => self.import_csv(&path),
            Action::ImportFromTxt { path } => self.import_txt(&path),
            Action::PreviewImport { path, options } => self.preview_import(&path, options),
            Action::ConfirmImport => self.confirm_import(),
            Action::CancelImport => {
                self.pending_import = None;
                Ok(())
            }
            Action::OpenProject { path } => self.open_project(&path),
            Action::SaveProject { path, dimensions } => self.save_project(&path, dimensions),
            Action::SavePlotStyle { path } => self.save_plot_style(&path),
            Action::ApplyPlotStyle { path } => self.apply_plot_style(&path),
            Action::SetChartTitle(new_title) => self.execute_command(Command::SetChartTitle {
                old: self.model.layout.title.clone(),
                new: new_title,
            }),
            Action::SetChartTitleVisible(visible) => {
                let mut next = self.model.layout.clone();
                next.title_visible = visible;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetAxisLabel { axis, label } => {
                if label.trim().is_empty() {
                    return self.fail(ControllerError::EmptyAxisLabel);
                }
                self.execute_command(Command::SetAxisLabel {
                    axis,
                    old: self.axis(axis).label.clone(),
                    new: label,
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
            Action::SetAxisTitleAnchor { axis, anchor } => {
                let old = self.axis(axis).clone();
                let mut new = old.clone();
                new.title_anchor = anchor
                    .map(|position| [position[0].clamp(0.0, 1.0), position[1].clamp(0.0, 1.0)]);
                self.execute_command(Command::ReplaceAxisConfig { axis, old, new })
            }
            Action::SetAxisScale { axis, scale } => {
                if scale != crate::model::ScaleType::Linear {
                    if let RangePolicy::Manual { min, max } = self.axis(axis).range {
                        if min <= 0.0 || max <= 0.0 {
                            return self.fail(ControllerError::InvalidRange { min, max });
                        }
                    }
                }
                self.execute_command(Command::SetAxisScale {
                    axis,
                    old: self.axis(axis).scale,
                    new: scale,
                })
            }
            Action::SetAxisRange { axis, range } => {
                if let RangePolicy::Manual { min, max } = range {
                    if !min.is_finite()
                        || !max.is_finite()
                        || min >= max
                        || (self.axis(axis).scale != crate::model::ScaleType::Linear
                            && (min <= 0.0 || max <= 0.0))
                    {
                        return self.fail(ControllerError::InvalidRange { min, max });
                    }
                }
                self.execute_command(Command::SetAxisRange {
                    axis,
                    old: self.axis(axis).range.clone(),
                    new: range,
                })
            }
            Action::SetAxesRanges { x, y } => {
                for (axis, range) in [(AxisKind::X, &x), (AxisKind::Y, &y)] {
                    if let RangePolicy::Manual { min, max } = range
                        && (!min.is_finite()
                            || !max.is_finite()
                            || min >= max
                            || (self.axis(axis).scale != ScaleType::Linear
                                && (*min <= 0.0 || *max <= 0.0)))
                    {
                        return self.fail(ControllerError::InvalidRange {
                            min: *min,
                            max: *max,
                        });
                    }
                }
                self.execute_command(Command::SetAxesRanges {
                    old_x: self.axis(AxisKind::X).range.clone(),
                    new_x: x,
                    old_y: self.axis(AxisKind::Y).range.clone(),
                    new_y: y,
                })
            }
            Action::EndAxesGesture => {
                self.coalesce_axes_ranges = false;
                Ok(())
            }
            Action::BeginContinuousEdit => {
                if self.continuous_edit_baseline.is_none() {
                    self.continuous_edit_baseline = Some(self.model.clone());
                }
                Ok(())
            }
            Action::EndContinuousEdit => self.finish_continuous_edit(),
            Action::SetAxisMajorTickStep { axis, step } => {
                self.execute_command(Command::SetAxisMajorTickStep {
                    axis,
                    old: self.axis(axis).ticks.major_step,
                    new: step,
                })
            }
            Action::SetAxisTickNumberFormat { axis, format } => {
                let old = self.axis(axis).clone();
                let mut new = old.clone();
                new.ticks.number_format = format;
                self.execute_command(Command::ReplaceAxisConfig { axis, old, new })
            }
            Action::SetAxisMinorTicks { axis, per_major } => {
                self.execute_command(Command::SetAxisMinorTicks {
                    axis,
                    old: self.axis(axis).ticks.minor_per_major,
                    new: per_major,
                })
            }
            Action::SetAxisTickMarkStyle {
                axis,
                minor,
                visible,
                length,
            } => {
                let old = self.axis(axis).clone();
                let mut new = old.clone();
                if minor {
                    new.ticks.minor_ticks_visible = visible;
                    new.ticks.minor_tick_length = length.clamp(0.0, 20.0);
                } else {
                    new.ticks.major_ticks_visible = visible;
                    new.ticks.major_tick_length = length.clamp(0.0, 20.0);
                }
                self.execute_command(Command::ReplaceAxisConfig { axis, old, new })
            }
            Action::SetAxisGridVisibility {
                axis,
                minor,
                visible,
            } => {
                let old = self.axis(axis).clone();
                let mut new = old.clone();
                if minor {
                    new.ticks.minor_grid_visible = visible;
                } else {
                    new.ticks.major_grid_visible = visible;
                }
                self.execute_command(Command::ReplaceAxisConfig { axis, old, new })
            }
            Action::SetAxisGridStyle {
                axis,
                minor,
                color,
                width,
            } => {
                let old = self.axis(axis).clone();
                let mut new = old.clone();
                if minor {
                    new.ticks.minor_grid_color = color;
                    new.ticks.minor_grid_width = width.max(0.25);
                } else {
                    new.ticks.major_grid_color = color;
                    new.ticks.major_grid_width = width.max(0.25);
                }
                self.execute_command(Command::ReplaceAxisConfig { axis, old, new })
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
                next.title = title.filter(|value| !value.trim().is_empty());
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendPosition(position) => {
                let mut next = self.model.legend.clone();
                next.position = position;
                next.anchor = match position {
                    LegendPosition::Custom => next.anchor,
                    LegendPosition::TopLeft => [0.04, 0.14],
                    LegendPosition::TopRight => [1.0, 0.14],
                    LegendPosition::BottomLeft => [0.04, 0.78],
                    LegendPosition::BottomRight => [1.0, 1.0],
                    _ => next.anchor,
                };
                self.execute_command(Command::ReplaceLegend {
                    old: self.model.legend.clone(),
                    new: next,
                })
            }
            Action::SetLegendAnchor(anchor) => {
                let mut next = self.model.legend.clone();
                next.position = LegendPosition::Custom;
                next.anchor = [anchor[0].clamp(0.0, 1.0), anchor[1].clamp(0.0, 1.0)];
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
            Action::SetLegendLayout {
                padding,
                row_spacing,
                sample_length,
                columns,
                column_spacing,
                background_color,
                border_color,
                border_width,
            } => {
                let mut next = self.model.legend.clone();
                next.padding = padding.min(48);
                next.row_spacing = row_spacing.min(48);
                next.sample_length = sample_length.clamp(4, 128);
                next.columns = columns.clamp(1, 8);
                next.column_spacing = column_spacing.min(64);
                next.background_color = background_color;
                next.border_color = border_color;
                next.border_width = border_width.clamp(0.0, 12.0);
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
            Action::SetPlotFontFamily(font_family) => {
                let mut next = self.model.layout.clone();
                next.font_family = font_family;
                self.execute_command(Command::ReplaceLayout {
                    old: self.model.layout.clone(),
                    new: next,
                })
            }
            Action::SetAnnotations(annotations) => {
                let mut ids = std::collections::HashSet::new();
                if annotations.iter().any(|annotation| {
                    annotation.id == 0
                        || !ids.insert(annotation.id)
                        || annotation.text.trim().is_empty()
                        || !annotation.x.is_finite()
                        || !annotation.y.is_finite()
                        || !(4..=256).contains(&annotation.font_size)
                        || (self.model.axes.x.scale != ScaleType::Linear && annotation.x <= 0.0)
                        || (self.model.axes.y.scale != ScaleType::Linear && annotation.y <= 0.0)
                }) {
                    return self.fail(ControllerError::ProjectFailed(
                        "Annotations need unique IDs, non-empty text, finite coordinates, and font sizes from 4 to 256".to_owned(),
                    ));
                }
                self.execute_command(Command::ReplaceAnnotations {
                    old: self.model.annotations.clone(),
                    new: annotations,
                })
            }
            Action::SetPanels(panels) => {
                let mut ids = std::collections::HashSet::new();
                if panels.len() > 12
                    || panels.iter().any(|panel| {
                        let mut assigned = std::collections::HashSet::new();
                        panel.id == 0
                            || !ids.insert(panel.id)
                            || panel.title.trim().is_empty()
                            || panel.series_ids.iter().any(|series_id| {
                                !assigned.insert(*series_id)
                                    || !self
                                        .model
                                        .series
                                        .iter()
                                        .any(|series| series.id == *series_id)
                            })
                    })
                {
                    return self.fail(ControllerError::ProjectFailed(
                        "Panels need unique IDs, non-empty titles, existing series, and a maximum of 12 panels".to_owned(),
                    ));
                }
                self.execute_command(Command::ReplacePanels {
                    old: self.model.panels.clone(),
                    new: panels,
                })
            }
            Action::AddSeries {
                name,
                x_column,
                y_column,
            } => {
                let (default_x, default_y) = self.default_xy_columns()?;
                let x_final = if x_column.is_empty() {
                    default_x
                } else {
                    x_column
                };
                let y_final = if y_column.is_empty() {
                    default_y
                } else {
                    y_column
                };
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
                    x_error_column: None,
                    y_error_column: None,
                    x_error_lower_column: None,
                    x_error_upper_column: None,
                    y_error_lower_column: None,
                    y_error_upper_column: None,
                    style: SeriesStyle {
                        color: self.color_for_series(self.next_series_id),
                        line_visible: true,
                        line_width: 2.0,
                        line_style: LineStyle::Solid,
                        line_interpolation: crate::model::LineInterpolation::Linear,
                        area_baseline: None,
                        area_fill_visible: false,
                        marker: None,
                        marker_preset: MarkerStyle {
                            shape: crate::model::MarkerShape::Circle,
                            size: 3.0,
                        },
                    },
                    visible: true,
                };
                self.next_series_id += 1;
                let old_panels = self.model.panels.clone();
                let mut new_panels = old_panels.clone();
                if let Some(first_panel) = new_panels.first_mut() {
                    first_panel.series_ids.push(series.id);
                }
                self.execute_command(Command::AddSeries {
                    series,
                    index: self.model.series.len(),
                    old_panels,
                    new_panels,
                })
            }
            Action::RemoveSeries { series_id } => {
                if self.model.series.len() <= 1 {
                    return self.fail(ControllerError::CannotRemoveLastSeries);
                }
                let Some((index, series)) = self.find_series_index(series_id) else {
                    return self.fail(ControllerError::SeriesNotFound(series_id));
                };
                let old_panels = self.model.panels.clone();
                let mut new_panels = old_panels.clone();
                for panel in &mut new_panels {
                    panel.series_ids.retain(|id| *id != series_id);
                }
                self.execute_command(Command::RemoveSeries {
                    series,
                    index,
                    old_panels,
                    new_panels,
                })
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
            Action::SetSeriesXColumn {
                series_id,
                x_column,
            } => {
                self.ensure_column_exists(&x_column)?;
                let old = self.find_series(series_id)?.x_column.clone();
                self.execute_command(Command::SetSeriesXColumn {
                    series_id,
                    old,
                    new: x_column,
                })
            }
            Action::SetSeriesYColumn {
                series_id,
                y_column,
            } => {
                self.ensure_column_exists(&y_column)?;
                let old = self.find_series(series_id)?.y_column.clone();
                self.execute_command(Command::SetSeriesYColumn {
                    series_id,
                    old,
                    new: y_column,
                })
            }
            Action::SetSeriesErrorColumn {
                series_id,
                axis,
                column,
            } => {
                if let Some(column) = &column {
                    self.ensure_column_exists(column)?;
                    self.validate_error_column_values(column)?;
                }
                let series = self.find_series(series_id)?;
                let old = match axis {
                    AxisKind::X => series.x_error_column.clone(),
                    AxisKind::Y => series.y_error_column.clone(),
                };
                self.execute_command(Command::SetSeriesErrorColumn {
                    series_id,
                    axis,
                    old,
                    new: column,
                })
            }
            Action::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis,
                side,
                column,
            } => {
                if let Some(column) = &column {
                    self.ensure_column_exists(column)?;
                    self.validate_error_column_values(column)?;
                }
                let series = self.find_series(series_id)?;
                let old = match (axis, side) {
                    (AxisKind::X, ErrorBarSide::Lower) => series.x_error_lower_column.clone(),
                    (AxisKind::X, ErrorBarSide::Upper) => series.x_error_upper_column.clone(),
                    (AxisKind::Y, ErrorBarSide::Lower) => series.y_error_lower_column.clone(),
                    (AxisKind::Y, ErrorBarSide::Upper) => series.y_error_upper_column.clone(),
                };
                self.execute_command(Command::SetSeriesAsymmetricErrorColumn {
                    series_id,
                    axis,
                    side,
                    old,
                    new: column,
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
            Action::SetSeriesLineVisible { series_id, visible } => {
                let old = self.find_series(series_id)?.style.line_visible;
                self.execute_command(Command::SetSeriesLineVisible {
                    series_id,
                    old,
                    new: visible,
                })
            }
            Action::SetSeriesLineStyle {
                series_id,
                line_style,
            } => {
                let old = self.find_series(series_id)?.style.line_style;
                self.execute_command(Command::SetSeriesLineStyle {
                    series_id,
                    old,
                    new: line_style,
                })
            }
            Action::SetSeriesLineInterpolation {
                series_id,
                interpolation,
            } => {
                let old = self.find_series(series_id)?.style.line_interpolation;
                self.execute_command(Command::SetSeriesLineInterpolation {
                    series_id,
                    old,
                    new: interpolation,
                })
            }
            Action::SetSeriesAreaBaseline {
                series_id,
                baseline,
            } => {
                if baseline.is_some_and(|value| !value.is_finite()) {
                    return self.fail(ControllerError::InvalidAreaBaseline(
                        baseline.unwrap_or_default(),
                    ));
                }
                let series = self.find_series(series_id)?;
                if baseline.is_some_and(|value| {
                    self.model.axes.y.scale != ScaleType::Linear && value <= 0.0
                }) {
                    return self.fail(ControllerError::InvalidAreaBaseline(
                        baseline.unwrap_or_default(),
                    ));
                }
                let old = series.style.area_baseline;
                let old_visible = series.style.area_fill_visible;
                let new_visible = baseline.is_some();
                self.execute_command(Command::SetSeriesAreaBaseline {
                    series_id,
                    old,
                    new: baseline,
                    old_visible,
                    new_visible,
                })
            }
            Action::SetSeriesAreaVisibility { series_id, visible } => {
                let series = self.find_series(series_id)?;
                let old = series.style.area_baseline;
                let old_visible = series.style.area_fill_visible;
                let new = if visible && old.is_none() {
                    Some(if self.model.axes.y.scale == ScaleType::Linear {
                        0.0
                    } else {
                        self.suggested_axis_range(AxisKind::Y)
                            .map(|(minimum, _)| minimum)
                            .unwrap_or(1.0)
                    })
                } else {
                    old
                };
                self.execute_command(Command::SetSeriesAreaBaseline {
                    series_id,
                    old,
                    new,
                    old_visible,
                    new_visible: visible,
                })
            }
            Action::SetSeriesMarker {
                series_id,
                marker,
                size,
            } => {
                let series = self.find_series(series_id)?;
                let old = series.style.marker.clone();
                let old_preset = series.style.marker_preset.clone();
                let new = marker.map(|shape| MarkerStyle { shape, size });
                let new_preset = new
                    .clone()
                    .or_else(|| old.clone())
                    .unwrap_or_else(|| old_preset.clone());
                self.execute_command(Command::SetSeriesMarker {
                    series_id,
                    old,
                    new,
                    old_preset,
                    new_preset,
                })
            }
            Action::RequestSaveAs { path, format, size } => {
                self.export_plot(&path, format, size, None)
            }
            Action::RequestPublicationExport {
                path,
                format,
                dimensions,
            } => {
                let size = dimensions
                    .pixel_size()
                    .map_err(ControllerError::ExportFailed)?;
                self.export_plot(&path, format, size, Some(dimensions))
            }
            Action::ReportUiError(message) => self.fail(ControllerError::ExportFailed(message)),
            Action::Undo => {
                self.finish_continuous_edit()?;
                self.undo()
            }
            Action::Redo => {
                self.finish_continuous_edit()?;
                self.redo()
            }
            other => self.fail(ControllerError::UnsupportedAction(match other {
                Action::ResetPlot => "ResetPlot",
                _ => "UnknownAction",
            })),
        };
        result
    }

    fn import_csv(&mut self, path: &str) -> Result<(), ControllerError> {
        let table =
            DataTable::from_csv_path(Path::new(path)).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table, source_file_name(path))
    }

    fn import_txt(&mut self, path: &str) -> Result<(), ControllerError> {
        let table =
            DataTable::from_txt_path(Path::new(path)).map_err(ControllerError::DataLoadFailed)?;
        self.set_table(table, source_file_name(path))
    }

    fn preview_import(
        &mut self,
        path: &str,
        options: ImportOptions,
    ) -> Result<(), ControllerError> {
        let preview = DataTable::preview_path_with_options(Path::new(path), options)
            .unwrap_or_else(|error| ImportPreview {
                columns: Vec::new(),
                sample_rows: Vec::new(),
                accepted_rows: 0,
                rejected_rows: 0,
                errors: Vec::new(),
                fatal_error: Some(error),
                table: None,
            });
        self.pending_import = Some(PendingImport {
            path: path.to_owned(),
            options,
            preview,
        });
        Ok(())
    }

    fn confirm_import(&mut self) -> Result<(), ControllerError> {
        let (table, source) = {
            let Some(pending) = &self.pending_import else {
                return self.fail(ControllerError::DataLoadFailed(
                    "There is no import preview to confirm".to_owned(),
                ));
            };
            if !pending.preview.can_import() {
                let explanation = pending
                    .preview
                    .fatal_error
                    .clone()
                    .or_else(|| pending.preview.errors.first().cloned())
                    .unwrap_or_else(|| "The preview contains rejected rows".to_owned());
                return self.fail(ControllerError::DataLoadFailed(explanation));
            }
            (
                pending
                    .preview
                    .table
                    .clone()
                    .expect("validated preview has a table"),
                source_file_name(&pending.path),
            )
        };
        self.set_table(table, source)
    }

    fn save_project(
        &mut self,
        path: &str,
        publication_dimensions: PublicationDimensions,
    ) -> Result<(), ControllerError> {
        let Some(data) = self.data_table.as_ref() else {
            return self.fail(ControllerError::NoDataLoaded);
        };
        let mut model = self.model.clone();
        let font_asset = match &model.layout.font_family {
            crate::model::PlotFontFamily::Custom(family) => {
                let (asset_id, bytes) =
                    crate::font::font_asset_by_legacy_name(family).ok_or_else(|| {
                        ControllerError::ProjectFailed(format!(
                            "Font family '{family}' is not registered and cannot be embedded"
                        ))
                    })?;
                model.layout.font_family = crate::model::PlotFontFamily::CustomAsset {
                    family: family.clone(),
                    asset_id,
                };
                Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes,
                ))
            }
            crate::model::PlotFontFamily::CustomAsset { family, asset_id } => {
                let bytes = crate::font::registered_font_asset(asset_id).ok_or_else(|| {
                    ControllerError::ProjectFailed(format!(
                        "Font asset '{asset_id}' for family '{family}' is not registered"
                    ))
                })?;
                Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes,
                ))
            }
            _ => None,
        };
        let project = crate::model::PlotProject::new(
            model,
            data.clone(),
            font_asset,
            self.data_source_name.clone().unwrap_or_default(),
            publication_dimensions,
        );
        project.validate().map_err(ControllerError::ProjectFailed)?;
        let contents = serde_json::to_vec_pretty(&project)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        crate::atomic_file::write_atomic(Path::new(path), &contents)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        self.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Project saved with embedded data and chart styling".to_owned(),
        });
        Ok(())
    }

    fn save_plot_style(&mut self, path: &str) -> Result<(), ControllerError> {
        let mut document = crate::model::PlotStyleDocument {
            format_version: 1,
            layout: self.model.layout.clone(),
            x_axis: crate::model::AxisStyle {
                axis_title_font_size: self.model.axes.x.axis_title_font_size,
                label_font_size: self.model.axes.x.label_font_size,
                ticks: self.model.axes.x.ticks.clone(),
            },
            y_axis: crate::model::AxisStyle {
                axis_title_font_size: self.model.axes.y.axis_title_font_size,
                label_font_size: self.model.axes.y.label_font_size,
                ticks: self.model.axes.y.ticks.clone(),
            },
            legend: self.model.legend.clone(),
            series: self
                .model
                .series
                .iter()
                .map(|series| {
                    let mut style = series.style.clone();
                    style.area_baseline = None;
                    style
                })
                .collect(),
            custom_font_asset_base64: None,
        };
        document.layout.title.clear();
        document.layout.title_visible = false;
        document.x_axis.ticks.major_step = None;
        document.y_axis.ticks.major_step = None;
        document.custom_font_asset_base64 = match &document.layout.font_family {
            crate::model::PlotFontFamily::Custom(family) => {
                let (_, bytes) = crate::font::font_asset_by_legacy_name(family).ok_or_else(|| {
                    ControllerError::ProjectFailed(format!(
                        "Font family '{family}' is not registered and cannot be included in a style"
                    ))
                })?;
                Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes,
                ))
            }
            crate::model::PlotFontFamily::CustomAsset { asset_id, .. } => {
                let bytes = crate::font::registered_font_asset(asset_id).ok_or_else(|| {
                    ControllerError::ProjectFailed(format!(
                        "Font asset '{asset_id}' is not registered and cannot be included in a style"
                    ))
                })?;
                Some(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    bytes,
                ))
            }
            _ => None,
        };
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        crate::atomic_file::write_atomic(Path::new(path), &bytes)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        self.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: format!("Plot style saved to {}", source_file_name(path)),
        });
        Ok(())
    }

    fn apply_plot_style(&mut self, path: &str) -> Result<(), ControllerError> {
        let bytes = std::fs::read(path).map_err(|error| {
            ControllerError::ProjectFailed(format!("Could not read plot style: {error}"))
        })?;
        let mut style: crate::model::PlotStyleDocument =
            serde_json::from_slice(&bytes).map_err(|error| {
                ControllerError::ProjectFailed(format!("Invalid plot style: {error}"))
            })?;
        let valid_ticks = |ticks: &TickConfig| {
            ticks.minor_per_major <= 20
                && ticks.major_tick_length.is_finite()
                && (0.0..=20.0).contains(&ticks.major_tick_length)
                && ticks.minor_tick_length.is_finite()
                && (0.0..=20.0).contains(&ticks.minor_tick_length)
                && ticks.major_grid_width.is_finite()
                && (0.25..=4.0).contains(&ticks.major_grid_width)
                && ticks.minor_grid_width.is_finite()
                && (0.25..=4.0).contains(&ticks.minor_grid_width)
        };
        if style.format_version != 1
            || !(8..=200).contains(&style.x_axis.axis_title_font_size)
            || !(8..=200).contains(&style.x_axis.label_font_size)
            || !(8..=200).contains(&style.y_axis.axis_title_font_size)
            || !(8..=200).contains(&style.y_axis.label_font_size)
            || !(8..=72).contains(&style.layout.title_font_size)
            || !(8..=64).contains(&style.legend.font_size)
            || style.layout.margin > 256
            || style.layout.x_label_area_size > 512
            || style.layout.y_label_area_size > 512
            || style.legend.columns == 0
            || style.legend.columns > 8
            || style.legend.padding > 48
            || style.legend.row_spacing > 48
            || !(4..=128).contains(&style.legend.sample_length)
            || style.legend.column_spacing > 64
            || !style.legend.border_width.is_finite()
            || !(0.0..=12.0).contains(&style.legend.border_width)
            || !valid_ticks(&style.x_axis.ticks)
            || !valid_ticks(&style.y_axis.ticks)
            || style.series.len() > 256
            || style.series.iter().any(|series| {
                !series.line_width.is_finite()
                    || !(1.0..=10.0).contains(&series.line_width)
                    || !series.marker_preset.size.is_finite()
                    || !(1.0..=16.0).contains(&series.marker_preset.size)
                    || series.marker.as_ref().is_some_and(|marker| {
                        !marker.size.is_finite() || !(1.0..=16.0).contains(&marker.size)
                    })
            })
        {
            return self.fail(ControllerError::ProjectFailed(
                "Plot style contains unsupported dimensions or settings".to_owned(),
            ));
        }
        match (&style.layout.font_family, &style.custom_font_asset_base64) {
            (crate::model::PlotFontFamily::CustomAsset { family, asset_id }, Some(encoded)) => {
                let bytes =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                        .map_err(|_| {
                            ControllerError::ProjectFailed(
                                "Style font asset is not valid base64".to_owned(),
                            )
                        })?;
                if crate::font::font_asset_id(&bytes) != *asset_id {
                    return self.fail(ControllerError::ProjectFailed(
                        "Style font fingerprint does not match its embedded font".to_owned(),
                    ));
                }
                crate::font::register_font_asset(family, bytes)
                    .map_err(ControllerError::ProjectFailed)?;
            }
            (crate::model::PlotFontFamily::Custom(family), Some(encoded)) => {
                let bytes =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                        .map_err(|_| {
                            ControllerError::ProjectFailed(
                                "Style font asset is not valid base64".to_owned(),
                            )
                        })?;
                let asset_id = crate::font::register_font_asset(family, bytes)
                    .map_err(ControllerError::ProjectFailed)?;
                style.layout.font_family = crate::model::PlotFontFamily::CustomAsset {
                    family: family.clone(),
                    asset_id,
                };
            }
            (crate::model::PlotFontFamily::Custom(_), None) => {
                return self.fail(ControllerError::ProjectFailed(
                    "Name-only custom fonts cannot be loaded from a portable style".to_owned(),
                ));
            }
            (crate::model::PlotFontFamily::CustomAsset { .. }, None) => {
                return self.fail(ControllerError::ProjectFailed(
                    "Portable style is missing its selected custom font".to_owned(),
                ));
            }
            (_, Some(_)) => {
                return self.fail(ControllerError::ProjectFailed(
                    "Style contains an unselected custom font asset".to_owned(),
                ));
            }
            _ => {}
        }

        let mut next = self.model.clone();
        next.layout.font_family = style.layout.font_family;
        next.layout.margin = style.layout.margin;
        next.layout.title_font_size = style.layout.title_font_size;
        next.layout.title_font_color = style.layout.title_font_color;
        next.layout.x_label_area_size = style.layout.x_label_area_size;
        next.layout.y_label_area_size = style.layout.y_label_area_size;
        for (axis, appearance) in [
            (&mut next.axes.x, style.x_axis),
            (&mut next.axes.y, style.y_axis),
        ] {
            axis.axis_title_font_size = appearance.axis_title_font_size;
            axis.label_font_size = appearance.label_font_size;
            let major_step = axis.ticks.major_step;
            axis.ticks = appearance.ticks;
            axis.ticks.major_step = major_step;
        }
        next.legend = style.legend;
        for (series, appearance) in next.series.iter_mut().zip(style.series) {
            let area_baseline = series.style.area_baseline;
            series.style = appearance;
            series.style.area_baseline = area_baseline;
            if series.style.area_fill_visible && area_baseline.is_none() {
                series.style.area_fill_visible = false;
            }
        }
        self.execute_command(Command::ReplacePlotModel {
            old: Box::new(self.model.clone()),
            new: Box::new(next),
        })?;
        self.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: format!("Plot style applied from {}", source_file_name(path)),
        });
        Ok(())
    }

    fn open_project(&mut self, path: &str) -> Result<(), ControllerError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        let mut project: crate::model::PlotProject = serde_json::from_str(&contents)
            .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
        project.validate().map_err(ControllerError::ProjectFailed)?;
        if project.format_version < 3 {
            for series in &mut project.model.series {
                series.style.area_fill_visible =
                    series.style.area_baseline.is_some_and(|baseline| {
                        project.model.axes.y.scale == ScaleType::Linear || baseline > 0.0
                    });
                if let Some(marker) = &series.style.marker {
                    series.style.marker_preset = marker.clone();
                }
            }
        }
        match project.model.layout.font_family.clone() {
            crate::model::PlotFontFamily::Custom(family) => {
                let encoded_asset = project.custom_font_asset_base64.as_ref().ok_or_else(|| {
                    ControllerError::ProjectFailed(
                        "Project references a custom font but does not embed its font asset"
                            .to_owned(),
                    )
                })?;
                let bytes = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    encoded_asset,
                )
                .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
                let asset_id = crate::font::register_font_asset(&family, bytes)
                    .map_err(ControllerError::ProjectFailed)?;
                project.model.layout.font_family =
                    crate::model::PlotFontFamily::CustomAsset { family, asset_id };
            }
            crate::model::PlotFontFamily::CustomAsset { family, asset_id } => {
                let encoded_asset = project.custom_font_asset_base64.as_ref().ok_or_else(|| {
                    ControllerError::ProjectFailed(
                        "Project references a custom font but does not embed its font asset"
                            .to_owned(),
                    )
                })?;
                let bytes = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    encoded_asset,
                )
                .map_err(|error| ControllerError::ProjectFailed(error.to_string()))?;
                let registered_id = crate::font::register_font_asset(&family, bytes)
                    .map_err(ControllerError::ProjectFailed)?;
                if registered_id != asset_id {
                    return self.fail(ControllerError::ProjectFailed(
                        "Project font asset fingerprint does not match its embedded file"
                            .to_owned(),
                    ));
                }
            }
            _ => {}
        }

        let next_series_id = project
            .model
            .series
            .iter()
            .map(|series| series.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        self.model = project.model;
        self.data_table = Some(project.data);
        self.data_revision = self.data_revision.saturating_add(1);
        self.data_source_name = Some(if project.source_name.is_empty() {
            source_file_name(path)
        } else {
            project.source_name
        });
        self.project_publication_dimensions = Some(project.publication_dimensions);
        self.pending_import = None;
        self.next_series_id = next_series_id;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.continuous_edit_baseline = None;
        self.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Project opened; data is embedded in the project".to_owned(),
        });
        Ok(())
    }

    fn set_table(&mut self, table: DataTable, source: String) -> Result<(), ControllerError> {
        let names = table.column_names();
        if names.len() < 2 {
            return self.fail(ControllerError::DataLoadFailed(
                "Need at least two numeric columns".to_owned(),
            ));
        }
        if self.model.series.is_empty() {
            self.model.series.push(SeriesModel {
                id: SeriesId(1),
                name: "Series 1".to_owned(),
                x_column: String::new(),
                y_column: String::new(),
                x_error_column: None,
                y_error_column: None,
                x_error_lower_column: None,
                x_error_upper_column: None,
                y_error_lower_column: None,
                y_error_upper_column: None,
                style: SeriesStyle {
                    color: Color {
                        r: 220,
                        g: 50,
                        b: 47,
                        a: 255,
                    },
                    line_visible: true,
                    line_width: 2.0,
                    line_style: LineStyle::Solid,
                    line_interpolation: crate::model::LineInterpolation::Linear,
                    area_baseline: None,
                    area_fill_visible: false,
                    marker: None,
                    marker_preset: MarkerStyle {
                        shape: crate::model::MarkerShape::Circle,
                        size: 3.0,
                    },
                },
                visible: true,
            });
        }

        let default_x = names[0].clone();
        let default_y = names[1].clone();
        let mut missing_bindings = Vec::new();
        for series in &mut self.model.series {
            if series.x_column.is_empty() {
                series.x_column = default_x.clone();
            }
            if series.y_column.is_empty() {
                series.y_column = default_y.clone();
            }
            for (role, column) in [
                ("X", Some(series.x_column.as_str())),
                ("Y", Some(series.y_column.as_str())),
                ("X uncertainty", series.x_error_column.as_deref()),
                ("Y uncertainty", series.y_error_column.as_deref()),
                (
                    "X lower uncertainty",
                    series.x_error_lower_column.as_deref(),
                ),
                (
                    "X upper uncertainty",
                    series.x_error_upper_column.as_deref(),
                ),
                (
                    "Y lower uncertainty",
                    series.y_error_lower_column.as_deref(),
                ),
                (
                    "Y upper uncertainty",
                    series.y_error_upper_column.as_deref(),
                ),
            ] {
                if let Some(column) = column
                    && !names.iter().any(|name| name == column)
                {
                    missing_bindings.push(format!("{} {role} → '{column}'", series.name));
                }
            }
        }

        for series in &self.model.series {
            let x_asymmetric =
                series.x_error_lower_column.is_some() || series.x_error_upper_column.is_some();
            let y_asymmetric =
                series.y_error_lower_column.is_some() || series.y_error_upper_column.is_some();
            let active_error_columns = [
                series.x_error_lower_column.as_deref(),
                series.x_error_upper_column.as_deref(),
                (!x_asymmetric)
                    .then_some(series.x_error_column.as_deref())
                    .flatten(),
                series.y_error_lower_column.as_deref(),
                series.y_error_upper_column.as_deref(),
                (!y_asymmetric)
                    .then_some(series.y_error_column.as_deref())
                    .flatten(),
            ];
            for column in active_error_columns.into_iter().flatten() {
                if table.has_column(column) {
                    validate_error_column_values_in_table(&table, column)?;
                }
            }
        }

        self.data_table = Some(table);
        self.data_revision = self.data_revision.saturating_add(1);
        self.data_source_name = Some(source.clone());
        self.project_publication_dimensions = None;
        self.pending_import = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.continuous_edit_baseline = None;
        self.notification = Some(Notification {
            level: if missing_bindings.is_empty() {
                NotificationLevel::Info
            } else {
                NotificationLevel::Error
            },
            message: if missing_bindings.is_empty() {
                format!("Loaded data from {source}")
            } else {
                format!(
                    "Loaded data from {source}. Check missing series columns: {}",
                    missing_bindings.join(", ")
                )
            },
        });
        Ok(())
    }

    fn default_xy_columns(&self) -> Result<(String, String), ControllerError> {
        let table = self
            .data_table
            .as_ref()
            .ok_or(ControllerError::NoDataLoaded)?;
        let names = table.column_names();
        if names.len() < 2 {
            return Err(ControllerError::DataLoadFailed(
                "Need at least two columns".to_owned(),
            ));
        }
        Ok((names[0].clone(), names[1].clone()))
    }

    fn ensure_column_exists(&self, name: &str) -> Result<(), ControllerError> {
        let table = self
            .data_table
            .as_ref()
            .ok_or(ControllerError::NoDataLoaded)?;
        if table.has_column(name) {
            Ok(())
        } else {
            Err(ControllerError::ColumnNotFound(name.to_owned()))
        }
    }

    fn validate_error_column_values(&self, name: &str) -> Result<(), ControllerError> {
        let table = self
            .data_table
            .as_ref()
            .ok_or(ControllerError::NoDataLoaded)?;
        validate_error_column_values_in_table(table, name)
    }

    fn export_plot(
        &self,
        path: &str,
        format: ImageFormat,
        size: ImageSize,
        publication_dimensions: Option<PublicationDimensions>,
    ) -> Result<(), ControllerError> {
        crate::font::ensure_plotters_fonts();
        let destination = Path::new(path);
        let temporary = crate::atomic_file::TempSibling::new(destination)
            .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
        let temporary_path = temporary
            .path()
            .to_str()
            .ok_or_else(|| ControllerError::ExportFailed("Invalid output path".to_owned()))?
            .to_owned();
        match format {
            ImageFormat::Png => {
                let backend = BitMapBackend::new(&temporary_path, (size.width, size.height));
                let root = backend.into_drawing_area();
                self.draw_export_chart(&root)?;
                root.present()
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
                drop(root);
                if let Some(dimensions) = publication_dimensions {
                    let png = std::fs::read(temporary.path())
                        .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
                    let png = crate::export::set_png_dpi(&png, dimensions.dpi)
                        .map_err(ControllerError::ExportFailed)?;
                    std::fs::write(temporary.path(), png)
                        .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
                }
                temporary
                    .commit(destination)
                    .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
            }
            ImageFormat::Svg => {
                let backend = SVGBackend::new(&temporary_path, (size.width, size.height));
                let root = backend.into_drawing_area();
                self.draw_export_chart(&root)?;
                root.present()
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
                drop(root);
                let svg = std::fs::read_to_string(temporary.path())
                    .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;
                let svg = if let Some(dimensions) = publication_dimensions {
                    crate::export::set_svg_physical_size(&svg, dimensions)
                        .map_err(ControllerError::ExportFailed)?
                } else {
                    svg
                };
                let font_family = self.model.layout.font_family.plotters_name();
                let svg = crate::font::embed_font_in_svg(&svg, &font_family)
                    .map_err(ControllerError::ExportFailed)?;
                crate::atomic_file::write_atomic(destination, svg.as_bytes())
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
            let (scaled, line_segments) = scale_points_into_segments(
                &points,
                self.model.axes.x.scale,
                self.model.axes.y.scale,
            );
            let error_bars = self.error_bars_for_series(series.id)?;
            rendered.push((series, scaled, line_segments, error_bars));
        }

        let data_points = || {
            rendered
                .iter()
                .filter(|(series, _, _, _)| {
                    self.model.panels.is_empty()
                        || self
                            .model
                            .panels
                            .iter()
                            .any(|panel| panel.series_ids.contains(&series.id))
                })
                .flat_map(|(series, points, _, errors)| {
                    let area_baseline = series
                        .style
                        .area_fill_visible
                        .then_some(series.style.area_baseline)
                        .flatten()
                        .and_then(|value| scale_value(value, self.model.axes.y.scale));
                    points.iter().copied().chain(
                        errors.bars.iter().flat_map(ErrorBar::range_points).chain(
                            area_baseline.into_iter().flat_map(|baseline| {
                                points.iter().map(move |(x, _)| (*x, baseline))
                            }),
                        ),
                    )
                })
        };
        let x_range = resolve_data_range(
            &self.model.axes.x.range,
            data_points(),
            true,
            -1.0..1.0,
            self.model.axes.x.scale,
        );
        let y_range = resolve_data_range(
            &self.model.axes.y.range,
            data_points(),
            false,
            -1.0..1.0,
            self.model.axes.y.scale,
        );

        let font_family = self.model.layout.font_family.plotters_name();
        let (effective_x_label_area, effective_y_label_area) = resolve_label_areas(
            &y_range,
            &self.model.axes.y.ticks,
            self.model.axes.y.scale,
            &font_family,
            &self.model.axes.x.label,
            &self.model.axes.y.label,
            self.model.layout.x_label_area_size,
            self.model.layout.y_label_area_size,
            self.model.axes.x.label_font_size,
            self.model.axes.y.label_font_size,
            self.model.axes.x.axis_title_font_size,
            self.model.axes.y.axis_title_font_size,
        );

        let legend_names = rendered
            .iter()
            .filter(|(series, points, _, _)| {
                !points.is_empty()
                    && (self.model.panels.is_empty()
                        || self
                            .model
                            .panels
                            .iter()
                            .any(|panel| panel.series_ids.contains(&series.id)))
            })
            .map(|(series, _, _, _)| series.name.as_str())
            .collect::<Vec<_>>();
        let chart_top_padding =
            if self.model.layout.title_visible && !self.model.layout.title.trim().is_empty() {
                let marker_clearance = rendered
                    .iter()
                    .filter(|(series, _, _, _)| {
                        self.model.panels.is_empty()
                            || self
                                .model
                                .panels
                                .iter()
                                .any(|panel| panel.series_ids.contains(&series.id))
                    })
                    .filter_map(|(series, _, _, _)| {
                        series
                            .style
                            .marker
                            .as_ref()
                            .map(|marker| marker.size.ceil().max(0.0) as u32 * 2)
                    })
                    .max()
                    .unwrap_or_default();
                self.model
                    .layout
                    .title_font_size
                    .saturating_add(marker_clearance)
            } else {
                0
            };
        let chart_margins = chart_margins_for_legend(
            self.model.layout.margin,
            &self.model.legend,
            &legend_names,
            chart_top_padding,
            effective_x_label_area,
        );
        let render_config = ChartRenderConfig::from_model(
            &self.model,
            x_range.clone(),
            y_range.clone(),
            chart_margins,
            chart_top_padding,
            effective_x_label_area,
            effective_y_label_area,
        );
        if !self.model.panels.is_empty() {
            let panel_columns = (self.model.panels.len() as f32).sqrt().ceil() as usize;
            let panel_rows = self.model.panels.len().div_ceil(panel_columns.max(1));
            let cells = root.split_evenly((panel_rows, panel_columns.max(1)));
            let title_color = RGBAColor(
                self.model.layout.title_font_color.r,
                self.model.layout.title_font_color.g,
                self.model.layout.title_font_color.b,
                self.model.layout.title_font_color.a as f64 / 255.0,
            );
            for (index, (panel, cell)) in self.model.panels.iter().zip(cells.iter()).enumerate() {
                let title = if index == 0
                    && self.model.layout.title_visible
                    && !self.model.layout.title.trim().is_empty()
                {
                    format!("{} — {}", self.model.layout.title, panel.title)
                } else {
                    panel.title.clone()
                };
                let mut builder = ChartBuilder::on(cell);
                builder.caption(
                    title,
                    (font_family.as_str(), self.model.layout.title_font_size)
                        .into_font()
                        .color(&title_color),
                );
                let mut panel_chart = builder
                    .margin_top(chart_margins.top)
                    .margin_bottom(chart_margins.bottom)
                    .margin_left(chart_margins.left)
                    .margin_right(chart_margins.right)
                    .x_label_area_size(effective_x_label_area)
                    .y_label_area_size(effective_y_label_area)
                    .build_cartesian_2d(x_range.clone(), y_range.clone())
                    .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
                let panel_data = rendered
                    .iter()
                    .filter(|(series, _, _, _)| panel.series_ids.contains(&series.id))
                    .collect::<Vec<_>>();
                let series_to_draw = panel_data
                    .iter()
                    .map(|(series, points, line_segments, error_bars)| SeriesRender {
                        points,
                        line_segments,
                        error_bars: &error_bars.bars,
                        area_baseline: series
                            .style
                            .area_fill_visible
                            .then_some(series.style.area_baseline)
                            .flatten()
                            .and_then(|value| scale_value(value, self.model.axes.y.scale)),
                        color: series.style.color,
                        line_visible: series.style.line_visible,
                        line_width: series.style.line_width,
                        line_style: series.style.line_style,
                        line_interpolation: series.style.line_interpolation,
                        marker: series.style.marker.as_ref(),
                    })
                    .collect::<Vec<_>>();
                draw_chart_content(cell, &mut panel_chart, &render_config, &series_to_draw)
                    .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
            }
            if self.model.legend.visible {
                let items = rendered
                    .iter()
                    .filter(|(series, points, _, _)| {
                        !points.is_empty()
                            && self
                                .model
                                .panels
                                .iter()
                                .any(|panel| panel.series_ids.contains(&series.id))
                    })
                    .map(|(series, _, _, _)| LegendItem {
                        name: &series.name,
                        color: series.style.color,
                        line_visible: series.style.line_visible,
                        line_style: series.style.line_style,
                        line_interpolation: series.style.line_interpolation,
                        area_fill: series.style.area_fill_visible,
                        marker: series.style.marker.as_ref(),
                    })
                    .collect::<Vec<_>>();
                draw_custom_legend(
                    root,
                    self.model.legend.title.as_deref(),
                    &items,
                    &self.model.legend,
                    &font_family,
                    self.model.layout.margin,
                    chart_top_padding,
                )
                .map_err(|error| ControllerError::ExportFailed(error.to_string()))?;
            }
            return Ok(());
        }
        let mut builder = ChartBuilder::on(root);
        if self.model.layout.title_visible && !self.model.layout.title.trim().is_empty() {
            builder.caption(
                self.model.layout.title.clone(),
                (font_family.as_str(), self.model.layout.title_font_size)
                    .into_font()
                    .color(&RGBAColor(
                        self.model.layout.title_font_color.r,
                        self.model.layout.title_font_color.g,
                        self.model.layout.title_font_color.b,
                        self.model.layout.title_font_color.a as f64 / 255.0,
                    )),
            );
        }
        let mut chart = builder
            .margin_top(chart_margins.top)
            .margin_bottom(chart_margins.bottom)
            .margin_left(chart_margins.left)
            .margin_right(chart_margins.right)
            .x_label_area_size(effective_x_label_area)
            .y_label_area_size(effective_y_label_area)
            .build_cartesian_2d(x_range.clone(), y_range.clone())
            .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

        let series_to_draw = rendered
            .iter()
            .map(|(series, points, line_segments, error_bars)| SeriesRender {
                points,
                line_segments,
                error_bars: &error_bars.bars,
                area_baseline: series
                    .style
                    .area_fill_visible
                    .then_some(series.style.area_baseline)
                    .flatten()
                    .and_then(|value| scale_value(value, self.model.axes.y.scale)),
                color: series.style.color,
                line_visible: series.style.line_visible,
                line_width: series.style.line_width,
                line_style: series.style.line_style,
                line_interpolation: series.style.line_interpolation,
                marker: series.style.marker.as_ref(),
            })
            .collect::<Vec<_>>();
        draw_chart_content(root, &mut chart, &render_config, &series_to_draw)
            .map_err(|e| ControllerError::ExportFailed(e.to_string()))?;

        if self.model.legend.visible {
            let items = rendered
                .iter()
                .filter(|(_, points, _, _)| !points.is_empty())
                .map(|(series, _, _, _)| LegendItem {
                    name: &series.name,
                    color: series.style.color,
                    line_visible: series.style.line_visible,
                    line_style: series.style.line_style,
                    line_interpolation: series.style.line_interpolation,
                    area_fill: series.style.area_fill_visible,
                    marker: series.style.marker.as_ref(),
                })
                .collect::<Vec<_>>();
            draw_custom_legend(
                root,
                self.model.legend.title.as_deref(),
                &items,
                &self.model.legend,
                &font_family,
                self.model.layout.margin,
                chart_top_padding,
            )
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

    fn finish_continuous_edit(&mut self) -> Result<(), ControllerError> {
        if let Some(old) = self.continuous_edit_baseline.take()
            && old != self.model
        {
            self.undo_stack.push(Command::ReplacePlotModel {
                old: Box::new(old),
                new: Box::new(self.model.clone()),
            });
            self.redo_stack.clear();
        }
        Ok(())
    }

    fn execute_command(&mut self, command: Command) -> Result<(), ControllerError> {
        let previous_model = self.model.clone();
        self.apply_command(&command);
        if self.model == previous_model {
            return Ok(());
        }
        if self.continuous_edit_baseline.is_some() {
            return Ok(());
        }
        if let Command::SetAxesRanges { new_x, new_y, .. } = &command {
            let mut merged = false;
            if self.coalesce_axes_ranges
                && let Some(Command::SetAxesRanges {
                    new_x: previous_x,
                    new_y: previous_y,
                    ..
                }) = self.undo_stack.last_mut()
            {
                *previous_x = new_x.clone();
                *previous_y = new_y.clone();
                merged = true;
            }
            self.coalesce_axes_ranges = true;
            if merged {
                self.redo_stack.clear();
                return Ok(());
            }
        } else {
            self.coalesce_axes_ranges = false;
        }
        self.undo_stack.push(command);
        self.redo_stack.clear();
        Ok(())
    }

    fn apply_command(&mut self, command: &Command) {
        match command {
            Command::ReplacePlotModel { new, .. } => self.model = (**new).clone(),
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
            Command::SetAxesRanges { new_x, new_y, .. } => {
                self.axis_mut(AxisKind::X).range = new_x.clone();
                self.axis_mut(AxisKind::Y).range = new_y.clone();
            }
            Command::SetAxisMajorTickStep { axis, new, .. } => {
                self.axis_mut(*axis).ticks.major_step = *new
            }
            Command::SetAxisMinorTicks { axis, new, .. } => {
                self.axis_mut(*axis).ticks.minor_per_major = *new
            }
            Command::ReplaceAxisConfig { axis, new, .. } => *self.axis_mut(*axis) = new.clone(),
            Command::AddSeries {
                series,
                index,
                new_panels,
                ..
            } => {
                self.model.series.insert(*index, series.clone());
                self.model.panels = new_panels.clone();
            }
            Command::RemoveSeries {
                index, new_panels, ..
            } => {
                self.model.series.remove(*index);
                self.model.panels = new_panels.clone();
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
            Command::SetSeriesErrorColumn {
                series_id,
                axis,
                new,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    match axis {
                        AxisKind::X => s.x_error_column = new.clone(),
                        AxisKind::Y => s.y_error_column = new.clone(),
                    }
                }
            }
            Command::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis,
                side,
                new,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    match (axis, side) {
                        (AxisKind::X, ErrorBarSide::Lower) => s.x_error_lower_column = new.clone(),
                        (AxisKind::X, ErrorBarSide::Upper) => s.x_error_upper_column = new.clone(),
                        (AxisKind::Y, ErrorBarSide::Lower) => s.y_error_lower_column = new.clone(),
                        (AxisKind::Y, ErrorBarSide::Upper) => s.y_error_upper_column = new.clone(),
                    }
                }
            }
            Command::SetSeriesLineWidth { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_width = *new;
                }
            }
            Command::SetSeriesLineVisible { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_visible = *new;
                }
            }
            Command::SetSeriesLineStyle { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_style = *new;
                }
            }
            Command::SetSeriesLineInterpolation { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_interpolation = *new;
                }
            }
            Command::SetSeriesAreaBaseline {
                series_id,
                new,
                new_visible,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.area_baseline = *new;
                    s.style.area_fill_visible = *new_visible;
                }
            }
            Command::SetSeriesColor { series_id, new, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.color = *new;
                }
            }
            Command::SetSeriesMarker {
                series_id,
                new,
                new_preset,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.marker = new.clone();
                    s.style.marker_preset = new_preset.clone();
                }
            }
            Command::ReplaceLegend { new, .. } => self.model.legend = new.clone(),
            Command::ReplaceLayout { new, .. } => self.model.layout = new.clone(),
            Command::ReplaceAnnotations { new, .. } => self.model.annotations = new.clone(),
            Command::ReplacePanels { new, .. } => self.model.panels = new.clone(),
            Command::Batch { commands } => {
                for c in commands {
                    self.apply_command(c);
                }
            }
        }
    }

    fn apply_inverse_command(&mut self, command: &Command) {
        match command {
            Command::ReplacePlotModel { old, .. } => self.model = (**old).clone(),
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
            Command::SetAxesRanges { old_x, old_y, .. } => {
                self.axis_mut(AxisKind::X).range = old_x.clone();
                self.axis_mut(AxisKind::Y).range = old_y.clone();
            }
            Command::SetAxisMajorTickStep { axis, old, .. } => {
                self.axis_mut(*axis).ticks.major_step = *old
            }
            Command::SetAxisMinorTicks { axis, old, .. } => {
                self.axis_mut(*axis).ticks.minor_per_major = *old
            }
            Command::ReplaceAxisConfig { axis, old, .. } => *self.axis_mut(*axis) = old.clone(),
            Command::AddSeries {
                index, old_panels, ..
            } => {
                self.model.series.remove(*index);
                self.model.panels = old_panels.clone();
            }
            Command::RemoveSeries {
                series,
                index,
                old_panels,
                ..
            } => {
                self.model.series.insert(*index, series.clone());
                self.model.panels = old_panels.clone();
            }
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
            Command::SetSeriesErrorColumn {
                series_id,
                axis,
                old,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    match axis {
                        AxisKind::X => s.x_error_column = old.clone(),
                        AxisKind::Y => s.y_error_column = old.clone(),
                    }
                }
            }
            Command::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis,
                side,
                old,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    match (axis, side) {
                        (AxisKind::X, ErrorBarSide::Lower) => s.x_error_lower_column = old.clone(),
                        (AxisKind::X, ErrorBarSide::Upper) => s.x_error_upper_column = old.clone(),
                        (AxisKind::Y, ErrorBarSide::Lower) => s.y_error_lower_column = old.clone(),
                        (AxisKind::Y, ErrorBarSide::Upper) => s.y_error_upper_column = old.clone(),
                    }
                }
            }
            Command::SetSeriesLineWidth { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_width = *old;
                }
            }
            Command::SetSeriesLineVisible { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_visible = *old;
                }
            }
            Command::SetSeriesLineStyle { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_style = *old;
                }
            }
            Command::SetSeriesLineInterpolation { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.line_interpolation = *old;
                }
            }
            Command::SetSeriesAreaBaseline {
                series_id,
                old,
                old_visible,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.area_baseline = *old;
                    s.style.area_fill_visible = *old_visible;
                }
            }
            Command::SetSeriesColor { series_id, old, .. } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.color = *old;
                }
            }
            Command::SetSeriesMarker {
                series_id,
                old,
                old_preset,
                ..
            } => {
                if let Some(s) = self.find_series_mut_raw(*series_id) {
                    s.style.marker = old.clone();
                    s.style.marker_preset = old_preset.clone();
                }
            }
            Command::ReplaceLegend { old, .. } => self.model.legend = old.clone(),
            Command::ReplaceLayout { old, .. } => self.model.layout = old.clone(),
            Command::ReplaceAnnotations { old, .. } => self.model.annotations = old.clone(),
            Command::ReplacePanels { old, .. } => self.model.panels = old.clone(),
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
            0 => Color {
                r: 220,
                g: 50,
                b: 47,
                a: 255,
            },
            1 => Color {
                r: 38,
                g: 139,
                b: 210,
                a: 255,
            },
            2 => Color {
                r: 133,
                g: 153,
                b: 0,
                a: 255,
            },
            3 => Color {
                r: 203,
                g: 75,
                b: 22,
                a: 255,
            },
            4 => Color {
                r: 42,
                g: 161,
                b: 152,
                a: 255,
            },
            _ => Color {
                r: 108,
                g: 113,
                b: 196,
                a: 255,
            },
        }
    }
}

impl Default for PlotController {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PlotModel {
    fn default() -> Self {
        Self {
            axes: AxesConfig {
                x: AxisConfig {
                    label: "X".to_owned(),
                    title_anchor: None,
                    axis_title_font_size: 18,
                    label_font_size: 16,
                    scale: ScaleType::Linear,
                    range: RangePolicy::Auto,
                    ticks: TickConfig {
                        major_step: None,
                        number_format: crate::model::TickNumberFormat::Auto,
                        minor_per_major: 1,
                        major_ticks_visible: true,
                        minor_ticks_visible: false,
                        major_tick_length: 5.0,
                        minor_tick_length: 3.0,
                        major_grid_visible: true,
                        minor_grid_visible: true,
                        major_grid_color: Color {
                            r: 190,
                            g: 190,
                            b: 190,
                            a: 255,
                        },
                        major_grid_width: 1.0,
                        minor_grid_color: Color {
                            r: 225,
                            g: 225,
                            b: 225,
                            a: 255,
                        },
                        minor_grid_width: 0.5,
                    },
                },
                y: AxisConfig {
                    label: "Y".to_owned(),
                    title_anchor: None,
                    axis_title_font_size: 18,
                    label_font_size: 16,
                    scale: ScaleType::Linear,
                    range: RangePolicy::Auto,
                    ticks: TickConfig {
                        major_step: None,
                        number_format: crate::model::TickNumberFormat::Auto,
                        minor_per_major: 1,
                        major_ticks_visible: true,
                        minor_ticks_visible: false,
                        major_tick_length: 5.0,
                        minor_tick_length: 3.0,
                        major_grid_visible: true,
                        minor_grid_visible: true,
                        major_grid_color: Color {
                            r: 190,
                            g: 190,
                            b: 190,
                            a: 255,
                        },
                        major_grid_width: 1.0,
                        minor_grid_color: Color {
                            r: 225,
                            g: 225,
                            b: 225,
                            a: 255,
                        },
                        minor_grid_width: 0.5,
                    },
                },
            },
            series: vec![SeriesModel {
                id: SeriesId(1),
                name: "Series 1".to_owned(),
                x_column: String::new(),
                y_column: String::new(),
                x_error_column: None,
                y_error_column: None,
                x_error_lower_column: None,
                x_error_upper_column: None,
                y_error_lower_column: None,
                y_error_upper_column: None,
                style: SeriesStyle {
                    color: Color {
                        r: 220,
                        g: 50,
                        b: 47,
                        a: 255,
                    },
                    line_visible: true,
                    line_width: 2.0,
                    line_style: LineStyle::Solid,
                    line_interpolation: crate::model::LineInterpolation::Linear,
                    area_baseline: None,
                    area_fill_visible: false,
                    marker: None,
                    marker_preset: MarkerStyle {
                        shape: crate::model::MarkerShape::Circle,
                        size: 3.0,
                    },
                },
                visible: true,
            }],
            legend: LegendConfig {
                visible: true,
                title: Some("Legend".to_owned()),
                position: LegendPosition::OutsideTop,
                font_size: 16,
                font_color: Color {
                    r: 20,
                    g: 20,
                    b: 20,
                    a: 255,
                },
                padding: 8,
                row_spacing: 8,
                sample_length: 24,
                columns: 1,
                column_spacing: 20,
                background_color: Color {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 230,
                },
                border_color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                border_width: 1.0,
                anchor: [0.72, 0.16],
            },
            layout: LayoutConfig {
                title: "Plot".to_owned(),
                title_visible: true,
                font_family: crate::model::PlotFontFamily::SansSerif,
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
            annotations: Vec::new(),
            panels: Vec::new(),
        }
    }
}

pub(crate) fn scale_value(value: f64, scale: ScaleType) -> Option<f64> {
    match scale {
        ScaleType::Linear => value.is_finite().then_some(value),
        ScaleType::Log10 | ScaleType::Log10Axis => {
            (value > 0.0 && value.is_finite()).then(|| value.log10())
        }
        ScaleType::LogE => (value > 0.0 && value.is_finite()).then(|| value.ln()),
    }
}

fn value_from_scale(value: f64, scale: ScaleType) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let value = match scale {
        ScaleType::Linear => value,
        ScaleType::Log10 | ScaleType::Log10Axis => 10.0_f64.powf(value),
        ScaleType::LogE => value.exp(),
    };
    value.is_finite().then_some(value)
}

fn apply_scale(x: f64, y: f64, x_scale: ScaleType, y_scale: ScaleType) -> Option<(f64, f64)> {
    Some((scale_value(x, x_scale)?, scale_value(y, y_scale)?))
}

pub(crate) fn scale_points_into_segments(
    points: &[(f64, f64)],
    x_scale: ScaleType,
    y_scale: ScaleType,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut scaled_points = Vec::with_capacity(points.len());
    let mut segments = Vec::new();
    let mut current_segment = Vec::new();

    for &(x, y) in points {
        if let Some(point) = apply_scale(x, y, x_scale, y_scale) {
            scaled_points.push(point);
            current_segment.push(point);
        } else if !current_segment.is_empty() {
            segments.push(std::mem::take(&mut current_segment));
        }
    }
    if !current_segment.is_empty() {
        segments.push(current_segment);
    }

    (scaled_points, segments)
}

fn error_bounds_for_point(
    value: f64,
    index: usize,
    symmetric: Option<&[f64]>,
    lower: Option<&[f64]>,
    upper: Option<&[f64]>,
    scale: ScaleType,
) -> Option<(f64, f64)> {
    if lower.is_some() || upper.is_some() {
        let lower = lower
            .and_then(|values| values.get(index))
            .copied()
            .unwrap_or(0.0);
        let upper = upper
            .and_then(|values| values.get(index))
            .copied()
            .unwrap_or(0.0);
        scaled_asymmetric_error_interval(value, lower, upper, scale)
    } else {
        symmetric
            .and_then(|values| values.get(index))
            .and_then(|error| scaled_error_interval(value, *error, scale))
    }
}

fn interval_has_non_positive_endpoint(
    value: f64,
    index: usize,
    symmetric: Option<&[f64]>,
    lower: Option<&[f64]>,
    upper: Option<&[f64]>,
) -> bool {
    let (lower_error, upper_error) = if lower.is_some() || upper.is_some() {
        let low = lower
            .and_then(|values| values.get(index))
            .copied()
            .unwrap_or(0.0);
        let high = upper
            .and_then(|values| values.get(index))
            .copied()
            .unwrap_or(0.0);
        (low, high)
    } else if let Some(error) = symmetric.and_then(|values| values.get(index)).copied() {
        (error, error)
    } else {
        return false;
    };
    lower_error.is_finite()
        && upper_error.is_finite()
        && lower_error >= 0.0
        && upper_error >= 0.0
        && (value - lower_error <= 0.0 || value + upper_error <= 0.0)
}

fn validate_error_column_values_in_table(
    table: &DataTable,
    name: &str,
) -> Result<(), ControllerError> {
    let values = table
        .column_values(name)
        .ok_or_else(|| ControllerError::ColumnNotFound(name.to_owned()))?;
    if let Some((row, value)) = values.iter().enumerate().find(|(_, value)| **value < 0.0) {
        return Err(ControllerError::DataLoadFailed(format!(
            "Uncertainty column '{name}' has a negative magnitude ({value}) at row {}. Error magnitudes must be non-negative.",
            row + 1
        )));
    }
    Ok(())
}

fn scaled_error_interval(value: f64, error: f64, scale: ScaleType) -> Option<(f64, f64)> {
    if !error.is_finite() || error < 0.0 {
        return None;
    }
    scaled_asymmetric_error_interval(value, error, error, scale)
}

fn scaled_asymmetric_error_interval(
    value: f64,
    lower_error: f64,
    upper_error: f64,
    scale: ScaleType,
) -> Option<(f64, f64)> {
    if !lower_error.is_finite()
        || !upper_error.is_finite()
        || lower_error < 0.0
        || upper_error < 0.0
    {
        return None;
    }
    let low = scale_value(value - lower_error, scale)?;
    let high = scale_value(value + upper_error, scale)?;
    Some((low.min(high), low.max(high)))
}

fn source_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}
