//! Main editor window composition.
//!
//! Dataflow: render from controller state, collect user interactions, emit actions.
//!
//! Architecture notes:
//! - This module is the egui-based View (no direct mutation of the Model). It reads from
//!   PlotController and emits Action values which the Controller translates to Commands.
//! - Plot drawing uses egui_plotter with Plotters backend. Shared Plotters rendering code
//!   draws axes, grid, labels, and series for both the preview callback and file export.
//! - Label areas are estimated centrally from formatted tick values and axis-title sizes.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::controller::action::Action;
use crate::controller::{NotificationLevel, PlotController};
use crate::model::{
    AxisKind, DecimalConvention, ErrorBarSide, HeaderMode, ImportDelimiter, LegendPosition,
    LineInterpolation, LineStyle, MarkerShape, PanelAxisView, PhysicalUnit, PlotFontFamily,
    PlotModel, PlotPanel, PublicationDimensions, RangePolicy, ScaleType, TickNumberFormat,
};
use crate::view::FilesMenu;
use crate::view::series::{
    ChartMargins, ChartRenderConfig, ErrorBarSet, LegendItem, SeriesRender, axis_title_geometry,
    chart_margins_for_legend, draw_chart_content, draw_custom_legend, legend_layout,
    resolve_data_range, resolve_label_areas,
};
use eframe::egui::{self, Color32, Ui};
use egui_plotter_fork::{Chart, MouseConfig};
use plotters::prelude::*;

#[derive(Clone)]
struct PreviewSeriesData {
    series: crate::model::SeriesModel,
    points: Vec<(f64, f64)>,
    line_segments: Vec<Vec<(f64, f64)>>,
    error_bars: ErrorBarSet,
    area_baseline: Option<f64>,
    confidence_band_segments: crate::controller::ConfidenceBandSegments,
}

#[derive(Clone)]
struct PreviewPanelData {
    panel: PlotPanel,
    axes: PanelAxisView,
    x_range: std::ops::Range<f64>,
    y_range: std::ops::Range<f64>,
    series: Vec<PreviewSeriesData>,
}

enum PendingDestructiveAction {
    OpenProject(String),
    ConfirmPrimaryImport,
}

/// EN: Main editor view. Contains menu, control panels and plot area.
/// RU: Osnovnoe predstavlenie. Soderzhit menyu, paneli upravleniya i oblast grafa.
pub struct PlotEditorView {
    export_dimensions: PublicationDimensions,
    match_export_aspect_ratio: bool,
    controls_collapsed: bool,
    text_drafts: HashMap<egui::Id, String>,
    selected_series: Option<crate::model::SeriesId>,
    selected_data_revision: Option<u64>,
    last_project_dimensions_revision: Option<u64>,
    inspected_point: Option<(crate::model::SeriesId, usize)>,
    pending_destructive_action: Option<PendingDestructiveAction>,
    #[cfg(test)]
    last_data_rect: Option<egui::Rect>,
    #[cfg(test)]
    last_panel_data_rects: Vec<egui::Rect>,
    #[cfg(test)]
    last_legend_rect: Option<egui::Rect>,
    #[cfg(test)]
    last_plot_rect: Option<egui::Rect>,
    #[cfg(test)]
    last_axis_title_hitboxes: Option<[(AxisKind, egui::Rect); 2]>,
    #[cfg(test)]
    last_panel_y_min_rect: Option<egui::Rect>,
}

impl PlotEditorView {
    /// Creates editor view state.
    pub fn new() -> Self {
        Self {
            export_dimensions: PublicationDimensions::default(),
            match_export_aspect_ratio: true,
            controls_collapsed: false,
            text_drafts: HashMap::new(),
            selected_series: None,
            selected_data_revision: None,
            last_project_dimensions_revision: None,
            inspected_point: None,
            pending_destructive_action: None,
            #[cfg(test)]
            last_data_rect: None,
            #[cfg(test)]
            last_panel_data_rects: Vec::new(),
            #[cfg(test)]
            last_legend_rect: None,
            #[cfg(test)]
            last_plot_rect: None,
            #[cfg(test)]
            last_axis_title_hitboxes: None,
            #[cfg(test)]
            last_panel_y_min_rect: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn inspected_point(&self) -> Option<(crate::model::SeriesId, usize)> {
        self.inspected_point
    }

    #[cfg(test)]
    pub(crate) fn selected_series(&self) -> Option<crate::model::SeriesId> {
        self.selected_series
    }

    #[cfg(test)]
    pub(crate) fn last_data_rect(&self) -> Option<egui::Rect> {
        self.last_data_rect
    }

    #[cfg(test)]
    pub(crate) fn last_panel_data_rects(&self) -> &[egui::Rect] {
        &self.last_panel_data_rects
    }

    #[cfg(test)]
    pub(crate) fn last_panel_y_min_rect(&self) -> Option<egui::Rect> {
        self.last_panel_y_min_rect
    }

    #[cfg(test)]
    pub(crate) fn last_legend_rect(&self) -> Option<egui::Rect> {
        self.last_legend_rect
    }

    #[cfg(test)]
    pub(crate) fn last_plot_rect(&self) -> Option<egui::Rect> {
        self.last_plot_rect
    }

    #[cfg(test)]
    pub(crate) fn export_dimensions(&self) -> PublicationDimensions {
        self.export_dimensions
    }

    #[cfg(test)]
    pub(crate) fn last_axis_title_hitboxes(&self) -> Option<[(AxisKind, egui::Rect); 2]> {
        self.last_axis_title_hitboxes
    }

    /// Renders full UI and collects actions requested by user interactions.
    pub fn draw(&mut self, ui: &mut Ui, controller: &PlotController) -> Vec<Action> {
        crate::font::ensure_plotters_fonts();
        crate::font::install_builtin_egui_fonts(ui.ctx());
        let data_revision = controller.data_revision();
        if self.last_project_dimensions_revision != Some(data_revision) {
            if let Some(dimensions) = controller.project_publication_dimensions() {
                self.export_dimensions = dimensions;
            }
            self.last_project_dimensions_revision = Some(data_revision);
        }
        let mut actions = Vec::new();
        for action in FilesMenu::draw(ui, self.export_dimensions, &mut self.controls_collapsed) {
            match action {
                Action::OpenProject { path } => {
                    if let Some(action) = self.request_open_project(path, controller) {
                        actions.push(action);
                    }
                }
                other => actions.push(other),
            }
        }
        self.draw_import_dialog(ui, controller, &mut actions);
        self.draw_unsaved_changes_dialog(ui, &mut actions);
        self.draw_controls(ui, controller, &mut actions);
        self.draw_plot(ui, controller, &mut actions);
        actions
    }

    fn draw_import_dialog(
        &mut self,
        ui: &mut Ui,
        controller: &PlotController,
        actions: &mut Vec<Action>,
    ) {
        let Some((path, current_options, preview)) = controller.import_preview() else {
            return;
        };
        let path = path.to_owned();
        let mut options = current_options;
        let mut options_changed = false;
        let mut confirm = false;
        let mut cancel = false;
        let add_table = controller.import_preview_adds_table();
        egui::Window::new(if add_table {
            "Add data table preview"
        } else {
            "Import preview"
        })
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .show(ui.ctx(), |ui| {
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&path);
                ui.label(file_name);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Delimiter")
                        .selected_text(import_delimiter_text(options.delimiter))
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                (ImportDelimiter::Comma, "Comma"),
                                (ImportDelimiter::Semicolon, "Semicolon"),
                                (ImportDelimiter::Tab, "Tab"),
                                (ImportDelimiter::Whitespace, "Whitespace"),
                            ] {
                                options_changed |= ui
                                    .selectable_value(&mut options.delimiter, value, label)
                                    .changed();
                            }
                        });
                    egui::ComboBox::from_label("Decimal")
                        .selected_text(match options.decimal {
                            DecimalConvention::Dot => "Dot",
                            DecimalConvention::Comma => "Comma",
                        })
                        .show_ui(ui, |ui| {
                            options_changed |= ui
                                .selectable_value(
                                    &mut options.decimal,
                                    DecimalConvention::Dot,
                                    "Dot (1.25)",
                                )
                                .changed();
                            options_changed |= ui
                                .selectable_value(
                                    &mut options.decimal,
                                    DecimalConvention::Comma,
                                    "Comma (1,25)",
                                )
                                .changed();
                        });
                    egui::ComboBox::from_label("Header row")
                        .selected_text(match options.header {
                            HeaderMode::Auto => "Auto",
                            HeaderMode::Present => "First row is header",
                            HeaderMode::Absent => "No header row",
                        })
                        .show_ui(ui, |ui| {
                            options_changed |= ui
                                .selectable_value(&mut options.header, HeaderMode::Auto, "Auto")
                                .changed();
                            options_changed |= ui
                                .selectable_value(
                                    &mut options.header,
                                    HeaderMode::Present,
                                    "First row is header",
                                )
                                .changed();
                            options_changed |= ui
                                .selectable_value(
                                    &mut options.header,
                                    HeaderMode::Absent,
                                    "No header row",
                                )
                                .changed();
                        });
                });

                if options.header == HeaderMode::Auto {
                    ui.small("Auto treats the first record as a header only when every cell looks like a name. A number or missing-value marker makes it data, so the row is checked for errors. Review the columns and sample.");
                }

                ui.label(format!(
                    "Valid rows: {}    Rejected rows: {}",
                    preview.accepted_rows, preview.rejected_rows
                ));
                ui.label("Missing or invalid values: reject the whole import; partial rows are never skipped.");
                if let Some(error) = &preview.fatal_error {
                    ui.colored_label(Color32::RED, error);
                }
                for error in preview.errors.iter().take(5) {
                    ui.colored_label(Color32::RED, error);
                }
                if !preview.columns.is_empty() {
                    egui::ScrollArea::both().max_height(240.0).show(ui, |ui| {
                        egui::Grid::new("import_preview_table")
                            .striped(true)
                            .show(ui, |ui| {
                                for column in &preview.columns {
                                    ui.strong(column);
                                }
                                ui.end_row();
                                for row in &preview.sample_rows {
                                    for value in row {
                                        ui.label(format_measurement(*value));
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            preview.can_import(),
                            egui::Button::new(if add_table { "Add table" } else { "Import data" }),
                        )
                        .clicked()
                    {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if options_changed {
            actions.push(if add_table {
                Action::PreviewAddTable { path, options }
            } else {
                Action::PreviewImport { path, options }
            });
        } else if confirm {
            if !add_table && self.has_unsaved_changes(controller) {
                self.pending_destructive_action =
                    Some(PendingDestructiveAction::ConfirmPrimaryImport);
            } else {
                actions.push(Action::ConfirmImport);
            }
        } else if cancel {
            actions.push(Action::CancelImport);
        }
    }

    fn has_unsaved_changes(&self, controller: &PlotController) -> bool {
        controller.has_unsaved_changes()
            || self.export_dimensions
                != controller
                    .project_publication_dimensions()
                    .unwrap_or_default()
    }

    pub(crate) fn request_open_project(
        &mut self,
        path: String,
        controller: &PlotController,
    ) -> Option<Action> {
        if self.has_unsaved_changes(controller) {
            self.pending_destructive_action = Some(PendingDestructiveAction::OpenProject(path));
            None
        } else {
            Some(Action::OpenProject { path })
        }
    }

    pub(crate) fn complete_pending_destructive_action(&mut self) {
        self.pending_destructive_action = None;
    }

    fn draw_unsaved_changes_dialog(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(pending) = &self.pending_destructive_action else {
            return;
        };
        let continuation = match pending {
            PendingDestructiveAction::OpenProject(path) => {
                Action::OpenProject { path: path.clone() }
            }
            PendingDestructiveAction::ConfirmPrimaryImport => Action::ConfirmImport,
        };
        let mut save_and_continue = false;
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label("Save the current plot and data before continuing?");
                ui.horizontal(|ui| {
                    if ui.button("Save and continue...").clicked() {
                        save_and_continue = true;
                    }
                    if ui.button("Discard changes").clicked() {
                        discard = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if save_and_continue {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("KiThePlot project", &["kplot"])
                .set_file_name("Untitled.kplot")
                .save_file()
            {
                let path = if path.extension().is_none() {
                    path.with_extension("kplot")
                } else {
                    path
                };
                actions.push(Action::SaveProjectThen {
                    path: path.display().to_string(),
                    dimensions: self.export_dimensions,
                    continuation: Box::new(continuation),
                });
            }
        } else if discard {
            self.pending_destructive_action = None;
            actions.push(continuation);
        } else if cancel {
            self.pending_destructive_action = None;
        }
    }

    fn draw_controls(
        &mut self,
        ui: &mut Ui,
        controller: &PlotController,
        actions: &mut Vec<Action>,
    ) {
        if self.controls_collapsed {
            return;
        }
        #[cfg(test)]
        {
            self.last_panel_y_min_rect = None;
        }
        egui::Panel::right("control_panel")
            .resizable(true)
            .default_size(360.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Controls");
                    ui.separator();

                    if self.selected_data_revision
                        .is_some_and(|revision| revision != controller.data_revision())
                    {
                        self.selected_series = None;
                        self.selected_data_revision = None;
                        self.inspected_point = None;
                    }

                    if let Some(n) = controller.notification() {
                        let color = match n.level {
                            NotificationLevel::Info => Color32::DARK_GREEN,
                            NotificationLevel::Error => Color32::RED,
                        };
                        ui.colored_label(color, &n.message);
                        ui.separator();
                    }
                    if let Some(summary) = controller.data_summary() {
                        ui.label(format!(
                            "Data: {} | longest column: {} points | {} columns",
                            summary.source, summary.max_column_len, summary.columns
                        ));
                        ui.separator();
                    }
                    if let Some(series_id) = self.selected_series {
                        if let Some(series) = controller
                            .model
                            .series
                            .iter()
                            .find(|series| series.id == series_id)
                        {
                            ui.group(|ui| {
                                ui.strong(format!("Selected series: {}", series.name));
                                ui.label(format!("X column: {}", series.x_column));
                                ui.label(format!("Y column: {}", series.y_column));
                                if let Some((point_series_id, row_index)) = self.inspected_point
                                    && point_series_id == series_id
                                {
                                    ui.separator();
                                    ui.strong("Selected point");
                                    match controller.points_for_series(series_id) {
                                        Ok(points) if row_index < points.len() => {
                                            let (x, y) = points[row_index];
                                        if let Some(source) = controller.table_name_for_series(series_id) {
                                            ui.label(format!("Source: {source}"));
                                        }
                                        ui.label(format!("Dataset row: {}", row_index + 1));
                                        ui.label(format!("X: {} = {}", series.x_column, format_measurement(x)));
                                        ui.label(format!("Y: {} = {}", series.y_column, format_measurement(y)));
                                        }
                                        _ => self.inspected_point = None,
                                    }
                                }
                            });
                            ui.separator();
                        } else {
                            self.selected_series = None;
                            self.inspected_point = None;
                        }
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Undo").clicked() {
                            actions.push(Action::Undo);
                        }
                        if ui.button("Redo").clicked() {
                            actions.push(Action::Redo);
                        }
                    });
                    ui.separator();

                    egui::CollapsingHeader::new("Axes")
                        .default_open(true)
                        .show(ui, |ui| {
                            axis_editor(
                                ui,
                                "X Axis",
                                AxisKind::X,
                                controller,
                                &mut self.text_drafts,
                                actions,
                            );
                            axis_editor(
                                ui,
                                "Y Axis",
                                AxisKind::Y,
                                controller,
                                &mut self.text_drafts,
                                actions,
                            );
                            if (matches!(controller.model.axes.x.range, RangePolicy::Manual { .. })
                                || matches!(
                                    controller.model.axes.y.range,
                                    RangePolicy::Manual { .. }
                                ))
                                && ui.button("Reset view to data range").clicked()
                            {
                                actions.push(Action::SetAxesRanges {
                                    x: RangePolicy::Auto,
                                    y: RangePolicy::Auto,
                                });
                            }
                        });

                    ui.collapsing("Legend", |ui| {
                        let mut legend_visible = controller.model.legend.visible;
                        if ui.checkbox(&mut legend_visible, "Visible").changed() {
                            actions.push(Action::SetLegendVisible(legend_visible));
                        }
                        let legend_title =
                            controller.model.legend.title.clone().unwrap_or_default();
                        if let Some(legend_title) = deferred_text_edit(
                            ui,
                            &mut self.text_drafts,
                            ui.make_persistent_id("legend_title"),
                            "Legend title",
                            &legend_title,
                        ) {
                            actions.push(Action::SetLegendTitle(
                                (!legend_title.is_empty()).then_some(legend_title),
                            ));
                        }
                        let mut legend_pos = controller.model.legend.position;
                        egui::ComboBox::from_label("Position")
                            .selected_text(legend_position_text(legend_pos))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::Custom,
                                    "Custom (drag)",
                                );
                                ui.separator();
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::TopLeft,
                                    "Top Left",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::TopRight,
                                    "Top Right",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::BottomLeft,
                                    "Bottom Left",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::BottomRight,
                                    "Bottom Right",
                                );
                                ui.separator();
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::OutsideLeft,
                                    "Outside Left",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::OutsideRight,
                                    "Outside Right",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::OutsideTop,
                                    "Outside Top",
                                );
                                ui.selectable_value(
                                    &mut legend_pos,
                                    LegendPosition::OutsideBottom,
                                    "Outside Bottom",
                                );
                            });
                        if legend_pos != controller.model.legend.position {
                            actions.push(Action::SetLegendPosition(legend_pos));
                        }
                        if legend_pos == LegendPosition::Custom {
                            let mut legend_anchor = controller.model.legend.anchor;
                            let (x_response, y_response) = ui
                                .horizontal(|ui| {
                                    ui.label("Legend X");
                                    let x_response = ui.add(egui::Slider::new(
                                        &mut legend_anchor[0],
                                        0.0..=1.0,
                                    ));
                                    ui.label("Y");
                                    let y_response = ui.add(egui::Slider::new(
                                        &mut legend_anchor[1],
                                        0.0..=1.0,
                                    ));
                                    (x_response, y_response)
                                })
                                .inner;
                            let changed = x_response.changed() || y_response.changed();
                            queue_gesture_action(
                                &[&x_response, &y_response],
                                changed.then_some(Action::SetLegendAnchor(legend_anchor)),
                                actions,
                            );
                        }
                        let mut legend_font_size = controller.model.legend.font_size;
                        let font_size_response = ui.add(
                            egui::Slider::new(&mut legend_font_size, 8..=64)
                                .text("Legend font size"),
                        );
                        queue_continuous_action(
                            &font_size_response,
                            font_size_response
                                .changed()
                                .then_some(Action::SetLegendFontSize(legend_font_size)),
                            actions,
                        );
                        let mut legend_color = color32_from_model(controller.model.legend.font_color);
                        let legend_color_response = color_edit_button(ui, "Legend font color", &mut legend_color);
                        queue_color_gesture_begin(ui, &legend_color_response, actions);
                        if legend_color_response.changed() {
                            actions.push(Action::SetLegendFontColor(model_color_from_ui(legend_color)));
                        }
                        queue_color_gesture_end(ui, &legend_color_response, actions);
                        let mut legend_padding = controller.model.legend.padding;
                        let mut legend_row_spacing = controller.model.legend.row_spacing;
                        let mut legend_sample_length = controller.model.legend.sample_length;
                        let mut legend_columns = controller.model.legend.columns;
                        let mut legend_column_spacing = controller.model.legend.column_spacing;
                        let mut legend_border_width = controller.model.legend.border_width;
                        let mut legend_background = color32_from_model(controller.model.legend.background_color);
                        let mut legend_border = color32_from_model(controller.model.legend.border_color);
                        let mut legend_layout_drag_stopped = false;
                        let mut legend_layout_changed = ui
                            .horizontal(|ui| {
                                ui.label("Padding");
                                let padding_response = ui
                                    .add(egui::DragValue::new(&mut legend_padding).range(0..=48));
                                if padding_response.drag_started() {
                                    actions.push(Action::BeginContinuousEdit);
                                }
                                legend_layout_drag_stopped |= padding_response.drag_stopped();
                                ui.label("Row gap");
                                let spacing_response = ui.add(
                                    egui::DragValue::new(&mut legend_row_spacing).range(0..=48),
                                );
                                if spacing_response.drag_started() {
                                    actions.push(Action::BeginContinuousEdit);
                                }
                                legend_layout_drag_stopped |= spacing_response.drag_stopped();
                                padding_response.changed() || spacing_response.changed()
                            })
                            .inner;
                        let sample_length_response = ui.add(
                            egui::Slider::new(&mut legend_sample_length, 4..=128)
                                .text("Sample length"),
                        );
                        if sample_length_response.drag_started() {
                            actions.push(Action::BeginContinuousEdit);
                        }
                        legend_layout_drag_stopped |= sample_length_response.drag_stopped();
                        legend_layout_changed |= sample_length_response.changed();
                        let columns_response = ui.add(
                            egui::Slider::new(&mut legend_columns, 1..=8).text("Columns"),
                        );
                        if columns_response.drag_started() {
                            actions.push(Action::BeginContinuousEdit);
                        }
                        legend_layout_drag_stopped |= columns_response.drag_stopped();
                        legend_layout_changed |= columns_response.changed();
                        let column_spacing_response = ui.add(
                            egui::Slider::new(&mut legend_column_spacing, 0..=64)
                                .text("Column gap"),
                        );
                        if column_spacing_response.drag_started() {
                            actions.push(Action::BeginContinuousEdit);
                        }
                        legend_layout_drag_stopped |= column_spacing_response.drag_stopped();
                        legend_layout_changed |= column_spacing_response.changed();
                        let border_width_response = ui.add(
                            egui::Slider::new(&mut legend_border_width, 0.0..=12.0)
                                .text("Border width"),
                        );
                        if border_width_response.drag_started() {
                            actions.push(Action::BeginContinuousEdit);
                        }
                        legend_layout_drag_stopped |= border_width_response.drag_stopped();
                        legend_layout_changed |= border_width_response.changed();
                        let background_response = color_edit_button(ui, "Legend background", &mut legend_background);
                        queue_color_gesture_begin(ui, &background_response, actions);
                        let legend_background_changed = background_response.changed();
                        let border_response = color_edit_button(ui, "Legend border color", &mut legend_border);
                        queue_color_gesture_begin(ui, &border_response, actions);
                        let legend_border_changed = border_response.changed();
                        legend_layout_changed |=
                            legend_background_changed || legend_border_changed;
                        if legend_layout_changed {
                            actions.push(Action::SetLegendLayout {
                                padding: legend_padding,
                                row_spacing: legend_row_spacing,
                                sample_length: legend_sample_length,
                                columns: legend_columns,
                                column_spacing: legend_column_spacing,
                                background_color: if legend_background_changed {
                                    model_color_from_ui(legend_background)
                                } else {
                                    controller.model.legend.background_color
                                },
                                border_color: if legend_border_changed {
                                    model_color_from_ui(legend_border)
                                } else {
                                    controller.model.legend.border_color
                                },
                                border_width: legend_border_width,
                            });
                        }
                        queue_color_gesture_end(ui, &background_response, actions);
                        queue_color_gesture_end(ui, &border_response, actions);
                        if legend_layout_drag_stopped {
                            actions.push(Action::EndContinuousEdit);
                        }
                    });

                    egui::CollapsingHeader::new("Chart title & typography")
                        .default_open(false)
                        .show(ui, |ui| {
                            if let Some(title) = deferred_text_edit(
                                ui,
                                &mut self.text_drafts,
                                ui.make_persistent_id("chart_title"),
                                "Chart title",
                                &controller.model.layout.title,
                            ) {
                                actions.push(Action::SetChartTitle(title));
                            }
                            let mut title_visible = controller.model.layout.title_visible;
                            if ui.checkbox(&mut title_visible, "Show title").changed() {
                                actions.push(Action::SetChartTitleVisible(title_visible));
                            }
                            let mut font_family = controller.model.layout.font_family.clone();
                            egui::ComboBox::from_label("Chart font")
                                .selected_text(font_family_text(&font_family))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut font_family,
                                        PlotFontFamily::SansSerif,
                                        "Sans Serif",
                                    );
                                    ui.selectable_value(
                                        &mut font_family,
                                        PlotFontFamily::Serif,
                                        "Serif",
                                    );
                                    ui.selectable_value(
                                        &mut font_family,
                                        PlotFontFamily::Monospace,
                                        "Monospace",
                                    );
                                    if let PlotFontFamily::Custom(name) = font_family.clone() {
                                        ui.selectable_value(
                                            &mut font_family,
                                            PlotFontFamily::Custom(name.clone()),
                                            format!("Custom: {name}"),
                                        );
                                    }
                                    for (name, asset_id) in crate::font::registered_chart_fonts() {
                                        let label = format!(
                                            "Custom: {name} ({})",
                                            &asset_id[..8.min(asset_id.len())]
                                        );
                                        ui.selectable_value(
                                            &mut font_family,
                                            PlotFontFamily::CustomAsset {
                                                family: name.clone(),
                                                asset_id,
                                            },
                                            label,
                                        );
                                    }
                                });
                            if font_family != controller.model.layout.font_family {
                                actions.push(Action::SetPlotFontFamily(font_family));
                            }
                            let mut title_font_size = controller.model.layout.title_font_size;
                            let title_font_response = ui.add(
                                egui::Slider::new(&mut title_font_size, 8..=72)
                                    .text("Title font size"),
                            );
                            queue_continuous_action(
                                &title_font_response,
                                title_font_response
                                    .changed()
                                    .then_some(Action::SetLabelFontSize(title_font_size)),
                                actions,
                            );
                            let mut title_color = color32_from_model(controller.model.layout.title_font_color);
                            let title_color_response = color_edit_button(ui, "Title color", &mut title_color);
                            queue_color_gesture_begin(ui, &title_color_response, actions);
                            if title_color_response.changed() {
                                actions.push(Action::SetLabelFontColor(model_color_from_ui(title_color)));
                            }
                            queue_color_gesture_end(ui, &title_color_response, actions);
                        });

                    egui::CollapsingHeader::new("Annotations")
                        .default_open(false)
                        .show(ui, |ui| {
                            let mut annotations = controller.model.annotations.clone();
                            let mut changed = false;
                            let mut annotation_gestures = Vec::new();
                            let mut annotation_color_gestures = Vec::new();
                            for annotation in &mut annotations {
                                ui.push_id(annotation.id, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.strong(format!("Annotation {}", annotation.id));
                                        if ui.small_button("Remove").clicked() {
                                            annotation.text.clear();
                                            changed = true;
                                        }
                                    });
                                    let label_id = ui.make_persistent_id("annotation-text");
                                    if let Some(text) = deferred_text_edit(
                                        ui,
                                        &mut self.text_drafts,
                                        label_id,
                                        "Text",
                                        &annotation.text,
                                    ) {
                                        annotation.text = text;
                                        changed = true;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label("X");
                                        let x = ui.add(egui::DragValue::new(&mut annotation.x).speed(0.1));
                                        ui.label("Y");
                                        let y = ui.add(egui::DragValue::new(&mut annotation.y).speed(0.1));
                                        changed |= x.changed() || y.changed();
                                        annotation_gestures.push(x);
                                        annotation_gestures.push(y);
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Font size");
                                        let size = ui.add(egui::DragValue::new(&mut annotation.font_size).range(4..=256));
                                        changed |= size.changed();
                                        annotation_gestures.push(size);
                                        let mut color = color32_from_model(annotation.color);
                                        let response = color_edit_button(ui, "Color", &mut color);
                                        if response.changed() {
                                            queue_color_gesture_begin(ui, &response, actions);
                                            annotation.color = model_color_from_ui(color);
                                            changed = true;
                                        }
                                        annotation_gestures.push(response.clone());
                                        annotation_color_gestures.push(response);
                                    });
                                });
                                ui.separator();
                            }
                            annotations.retain(|annotation| !annotation.text.is_empty());
                            if annotation_gestures.iter().any(egui::Response::drag_started) {
                                actions.push(Action::BeginContinuousEdit);
                            }
                            if changed {
                                actions.push(Action::SetAnnotations(annotations.clone()));
                            }
                            for response in &annotation_color_gestures {
                                queue_color_gesture_end(ui, response, actions);
                            }
                            if annotation_gestures.iter().any(egui::Response::drag_stopped) {
                                actions.push(Action::EndContinuousEdit);
                            }
                            if ui.button("Add annotation").clicked() {
                                let (x_min, x_max) = controller
                                    .suggested_axis_range(AxisKind::X)
                                    .unwrap_or((0.0, 1.0));
                                let (y_min, y_max) = controller
                                    .suggested_axis_range(AxisKind::Y)
                                    .unwrap_or((0.0, 1.0));
                                annotations.push(crate::model::PlotAnnotation {
                                    id: annotations.iter().map(|item| item.id).max().unwrap_or(0) + 1,
                                    text: "Annotation".to_owned(),
                                    x: (x_min + x_max) / 2.0,
                                    y: (y_min + y_max) / 2.0,
                                    font_size: 14,
                                    color: crate::model::Color { r: 20, g: 20, b: 20, a: 255 },
                                });
                                actions.push(Action::SetAnnotations(annotations));
                            }
                            if controller.model.annotations.iter().any(|annotation| annotation.text.trim().is_empty()) {
                                ui.colored_label(Color32::RED, "Annotation text cannot be empty.");
                            }
                            ui.small("Coordinates use the axis data units; annotations outside the visible range are hidden.");
                        });

                    egui::CollapsingHeader::new("Plot panels")
                        .default_open(false)
                        .show(ui, |ui| {
                            let mut panels = controller.model.panels.clone();
                            let mut changed = false;
                            let mut remove_id = None;
                            for panel in &mut panels {
                                ui.push_id(panel.id, |ui| {
                                    ui.horizontal(|ui| {
                                        let title_id = ui.make_persistent_id("panel-title");
                                        if let Some(title) = deferred_text_edit(
                                            ui,
                                            &mut self.text_drafts,
                                            title_id,
                                            "Panel title",
                                            &panel.title,
                                        ) {
                                            panel.title = title;
                                            changed = true;
                                        }
                                        if ui.small_button("Remove panel").clicked() {
                                            remove_id = Some(panel.id);
                                        }
                                    });
                                    ui.label("Series");
                                    for series in &controller.model.series {
                                        let mut assigned = panel.series_ids.contains(&series.id);
                                        if ui.checkbox(&mut assigned, &series.name).changed() {
                                            if assigned {
                                                panel.series_ids.push(series.id);
                                            } else {
                                                panel.series_ids.retain(|id| *id != series.id);
                                            }
                                            changed = true;
                                        }
                                    }
                                    let mut independent_axes = panel.axis_view.is_some();
                                    if ui
                                        .checkbox(&mut independent_axes, "Independent axes")
                                        .changed()
                                    {
                                        let view = independent_axes.then(|| {
                                            crate::model::PanelAxisView {
                                                x_scale: controller.model.axes.x.scale,
                                                y_scale: controller.model.axes.y.scale,
                                                x_range: controller.model.axes.x.range.clone(),
                                                y_range: controller.model.axes.y.range.clone(),
                                            }
                                        });
                                        actions.push(Action::SetPanelAxisView {
                                            panel_id: panel.id,
                                            view: view.clone(),
                                        });
                                        panel.axis_view = view;
                                    }
                                    if let Some(view) = &mut panel.axis_view {
                                        let y_min_rect = panel_axis_controls(
                                            ui, panel.id, view, controller, actions,
                                        );
                                        #[cfg(test)]
                                        if y_min_rect.is_some() {
                                            self.last_panel_y_min_rect = y_min_rect;
                                        }
                                        #[cfg(not(test))]
                                        let _ = y_min_rect;
                                    }
                                });
                                ui.separator();
                            }
                            if let Some(id) = remove_id {
                                panels.retain(|panel| panel.id != id);
                                changed = true;
                            }
                            if changed {
                                actions.push(Action::SetPanels(panels.clone()));
                            }
                            if ui.button("Add plot panel").clicked() {
                                let id = panels.iter().map(|panel| panel.id).max().unwrap_or(0) + 1;
                                let series_ids = if panels.is_empty() {
                                    controller.model.series.iter().map(|series| series.id).collect()
                                } else {
                                    Vec::new()
                                };
                                panels.push(crate::model::PlotPanel {
                                    id,
                                    title: format!("Panel {id}"),
                                    series_ids,
                                    axis_view: None,
                                });
                                actions.push(Action::SetPanels(panels));
                            }
                            ui.small("Panels share axes, ranges, zoom, and pan by default. Independent axes give each panel its own scale and range; zoom/pan then affects only the panel under the pointer.");
                        });

                    egui::CollapsingHeader::new("Series")
                        .default_open(false)
                        .show(ui, |ui| {
                            let tables = controller.data_tables();
                            for series in &controller.model.series {
                                let columns = controller.columns_for_series(series.id);
                                let series_rows = controller
                                    .column_len_for_series(series.id, &series.x_column)
                                    .unwrap_or_default();
                                let plotted_rows = if controller.model.panels.is_empty() {
                                    let count = controller
                                        .points_for_series(series.id)
                                        .map(|points| {
                                            crate::controller::scale_points_into_segments(
                                                &points,
                                                controller.model.axes.x.scale,
                                                controller.model.axes.y.scale,
                                            )
                                            .0
                                            .len()
                                        })
                                        .unwrap_or_default();
                                    format!("{count}/{series_rows}")
                                } else {
                                    let panel_counts = controller
                                        .model
                                        .panels
                                        .iter()
                                        .filter(|panel| panel.series_ids.contains(&series.id))
                                        .map(|panel| {
                                            let count = controller
                                                .effective_panel_axis_view(panel.id)
                                                .and_then(|axes| {
                                                    controller.points_for_series(series.id).ok().map(
                                                        |points| {
                                                            crate::controller::scale_points_into_segments(
                                                                &points,
                                                                axes.x_scale,
                                                                axes.y_scale,
                                                            )
                                                            .0
                                                            .len()
                                                        },
                                                    )
                                                })
                                                .unwrap_or_default();
                                            format!("{}: {count}/{series_rows}", panel.title)
                                        })
                                        .collect::<Vec<_>>();
                                    if panel_counts.is_empty() {
                                        "unassigned".to_owned()
                                    } else {
                                        panel_counts.join(", ")
                                    }
                                };
                                let visibility = if series.visible { "" } else { " · hidden" };
                                let heading = format!(
                                    "{} | {} -> {} | {plotted_rows} pts{visibility} (ID {})",
                                    series.name, series.x_column, series.y_column, series.id.0
                                );
                                egui::CollapsingHeader::new(heading)
                                    .id_salt(series.id.0)
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        ui.push_id(series.id.0, |ui| {
                                            ui.group(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(format!("ID {}", series.id.0));
                                                    if ui.button("Remove").clicked() {
                                                        actions.push(Action::RemoveSeries {
                                                            series_id: series.id,
                                                        });
                                                    }
                                                });

                                                if tables.len() > 1 {
                                                    let selected_name = controller
                                                        .table_name_for_series(series.id)
                                                        .unwrap_or_else(|| "Missing table".to_owned());
                                                    let mut selected_table = series.table_id;
                                                    egui::ComboBox::from_label("Data table")
                                                        .selected_text(selected_name)
                                                        .show_ui(ui, |ui| {
                                                            for (table_id, name) in &tables {
                                                                ui.selectable_value(
                                                                    &mut selected_table,
                                                                    *table_id,
                                                                    name,
                                                                );
                                                            }
                                                        });
                                                    if selected_table != series.table_id {
                                                        actions.push(Action::SetSeriesTable {
                                                            series_id: series.id,
                                                            table_id: selected_table,
                                                        });
                                                    }
                                                }

                                                if let Some(name) = deferred_text_edit(
                                                    ui,
                                                    &mut self.text_drafts,
                                                    ui.make_persistent_id("series_name"),
                                                    "Series name",
                                                    &series.name,
                                                ) {
                                                    actions.push(Action::RenameSeries {
                                                        series_id: series.id,
                                                        name,
                                                    });
                                                }

                                                let mut visible = series.visible;
                                                if ui.checkbox(&mut visible, "Visible").changed() {
                                                    actions.push(Action::SetSeriesVisibility {
                                                        series_id: series.id,
                                                        visible,
                                                    });
                                                }

                                                if columns.is_empty() {
                                                    ui.label("Load CSV/TXT to select X/Y columns");
                                                } else {
                                                    let x_asymmetric = series.x_error_lower_column.is_some()
                                                        || series.x_error_upper_column.is_some();
                                                    let y_asymmetric = series.y_error_lower_column.is_some()
                                                        || series.y_error_upper_column.is_some();
                                                    let valid_columns = columns
                                                        .iter()
                                                        .filter_map(|name| {
                                                            let count = controller
                                                                .column_len_for_series(series.id, name)?;
                                                            (count > 0).then(|| (name.clone(), count))
                                                        })
                                                        .collect::<Vec<_>>();
                                                    let mut columns_per_length = HashMap::new();
                                                    for (_, count) in &valid_columns {
                                                        *columns_per_length.entry(*count).or_insert(0usize) += 1;
                                                    }
                                                    let valid_x_columns = valid_columns
                                                        .iter()
                                                        .filter(|(_, count)| columns_per_length[count] > 1)
                                                        .collect::<Vec<_>>();
                                                    if valid_x_columns.is_empty() {
                                                        ui.colored_label(
                                                            Color32::RED,
                                                            "No non-empty X/Y column pair has matching lengths",
                                                        );
                                                    } else {
                                                        let mut selected_x = valid_x_columns
                                                            .iter()
                                                            .find(|(name, _)| name.as_str() == series.x_column)
                                                            .unwrap_or(&valid_x_columns[0])
                                                            .0
                                                            .clone();
                                                        let mut x_changed = false;
                                                        egui::ComboBox::from_label("X column")
                                                            .selected_text(&selected_x)
                                                            .show_ui(ui, |ui| {
                                                                for (name, count) in &valid_x_columns {
                                                                    x_changed |= ui
                                                                        .selectable_value(
                                                                            &mut selected_x,
                                                                            name.clone(),
                                                                            format!("{name} ({count} points)"),
                                                                        )
                                                                        .changed();
                                                                }
                                                            });
                                                        let selected_x_count = valid_columns
                                                            .iter()
                                                            .find(|(name, _)| name == &selected_x)
                                                            .map(|(_, count)| *count)
                                                            .unwrap_or_default();
                                                        let valid_y_columns = valid_columns
                                                            .iter()
                                                            .filter(|(name, count)| name != &selected_x && *count == selected_x_count)
                                                            .collect::<Vec<_>>();
                                                        let mut selected_y = valid_y_columns
                                                            .iter()
                                                            .find(|(name, _)| name.as_str() == series.y_column)
                                                            .unwrap_or(&valid_y_columns[0])
                                                            .0
                                                            .clone();
                                                        let mut y_changed = false;
                                                        egui::ComboBox::from_label("Y column")
                                                            .selected_text(&selected_y)
                                                            .show_ui(ui, |ui| {
                                                                for (name, count) in &valid_y_columns {
                                                                    y_changed |= ui
                                                                        .selectable_value(
                                                                            &mut selected_y,
                                                                            name.clone(),
                                                                            format!("{name} ({count} points)"),
                                                                        )
                                                                        .changed();
                                                                }
                                                            });
                                                        if (x_changed || y_changed)
                                                            && (selected_x != series.x_column || selected_y != series.y_column)
                                                        {
                                                            actions.push(Action::SetSeriesColumns {
                                                                series_id: series.id,
                                                                x_column: selected_x,
                                                                y_column: selected_y,
                                                            });
                                                        }
                                                    }
                                                    error_column_combo(
                                                        ui,
                                                        series.id,
                                                        AxisKind::X,
                                                        "X symmetric uncertainty",
                                                        series.x_error_column.clone(),
                                                        !x_asymmetric,
                                                        &columns,
                                                        actions,
                                                    );
                                                    error_column_combo(
                                                        ui,
                                                        series.id,
                                                        AxisKind::Y,
                                                        "Y symmetric uncertainty",
                                                        series.y_error_column.clone(),
                                                        !y_asymmetric,
                                                        &columns,
                                                        actions,
                                                    );
                                                    ui.small(format!(
                                                        "X intervals: {}",
                                                        if x_asymmetric { "asymmetric" } else { "symmetric" }
                                                    ));
                                                    ui.small(format!(
                                                        "Y intervals: {}",
                                                        if y_asymmetric { "asymmetric" } else { "symmetric" }
                                                    ));
                                                    ui.collapsing(
                                                        "Asymmetric uncertainties",
                                                        |ui| {
                                                            asymmetric_error_column_combo(
                                                                ui,
                                                                series.id,
                                                                AxisKind::X,
                                                                ErrorBarSide::Lower,
                                                                "X lower",
                                                                series.x_error_lower_column.clone(),
                                                                &columns,
                                                                actions,
                                                            );
                                                            asymmetric_error_column_combo(
                                                                ui,
                                                                series.id,
                                                                AxisKind::X,
                                                                ErrorBarSide::Upper,
                                                                "X upper",
                                                                series.x_error_upper_column.clone(),
                                                                &columns,
                                                                actions,
                                                            );
                                                            asymmetric_error_column_combo(
                                                                ui,
                                                                series.id,
                                                                AxisKind::Y,
                                                                ErrorBarSide::Lower,
                                                                "Y lower",
                                                                series.y_error_lower_column.clone(),
                                                                &columns,
                                                                actions,
                                                            );
                                                            asymmetric_error_column_combo(
                                                                ui,
                                                                series.id,
                                                                AxisKind::Y,
                                                                ErrorBarSide::Upper,
                                                                "Y upper",
                                                                series.y_error_upper_column.clone(),
                                                                &columns,
                                                                actions,
                                                            );
                                                        },
                                                    );
                                                    ui.collapsing(
                                                        "Confidence band (source columns)",
                                                        |ui| {
                                                            let original_lower = series
                                                                .confidence_band
                                                                .as_ref()
                                                                .map(|band| band.lower_column.as_str())
                                                                .unwrap_or_default();
                                                            let original_upper = series
                                                                .confidence_band
                                                                .as_ref()
                                                                .map(|band| band.upper_column.as_str())
                                                                .unwrap_or_default();
                                                            let mut lower = original_lower.to_owned();
                                                            let mut upper = original_upper.to_owned();
                                                            egui::ComboBox::from_label("Lower bound")
                                                                .selected_text(if lower.is_empty() { "Choose column" } else { &lower })
                                                                .show_ui(ui, |ui| {
                                                                    for column in &columns {
                                                                        ui.selectable_value(&mut lower, column.clone(), column);
                                                                    }
                                                                });
                                                            egui::ComboBox::from_label("Upper bound")
                                                                .selected_text(if upper.is_empty() { "Choose column" } else { &upper })
                                                                .show_ui(ui, |ui| {
                                                                    for column in &columns {
                                                                        ui.selectable_value(&mut upper, column.clone(), column);
                                                                    }
                                                                });
                                                            ui.horizontal(|ui| {
                                                                if ui
                                                                    .add_enabled(
                                                                        !lower.is_empty() && !upper.is_empty(),
                                                                        egui::Button::new("Apply band"),
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    actions.push(Action::SetSeriesConfidenceBand {
                                                                        series_id: series.id,
                                                                        band: Some(crate::model::ConfidenceBand {
                                                                            lower_column: lower.clone(),
                                                                            upper_column: upper.clone(),
                                                                        }),
                                                                    });
                                                                }
                                                                if series.confidence_band.is_some()
                                                                    && ui.button("Remove band").clicked()
                                                                {
                                                                    actions.push(Action::SetSeriesConfidenceBand {
                                                                        series_id: series.id,
                                                                        band: None,
                                                                    });
                                                                }
                                                            });
                                                            ui.small("The band is filled with the series color at 20% opacity. Bounds are validated row by row; non-positive bounds are split out on log axes.");
                                                        },
                                                    );
                                                }

                                                let mut line_visible = series.style.line_visible;
                                                if ui
                                                    .checkbox(&mut line_visible, "Connect points")
                                                    .changed()
                                                {
                                                    actions.push(Action::SetSeriesLineVisible {
                                                        series_id: series.id,
                                                        visible: line_visible,
                                                    });
                                                }
                                                let mut area_visible =
                                                    series.style.area_fill_visible;
                                                if ui
                                                    .checkbox(&mut area_visible, "Area fill")
                                                    .changed()
                                                {
                                                    actions.push(
                                                        Action::SetSeriesAreaVisibility {
                                                            series_id: series.id,
                                                            visible: area_visible,
                                                        },
                                                    );
                                                }
                                                if area_visible
                                                    && let Some(baseline) = series
                                                        .style
                                                        .area_baseline
                                                {
                                                    let mut baseline = baseline;
                                                    ui.horizontal(|ui| {
                                                        ui.label("Fill baseline");
                                                        let response = ui.add(
                                                            egui::DragValue::new(&mut baseline)
                                                                .speed(0.1),
                                                        );
                                                        queue_continuous_action(
                                                            &response,
                                                            response.changed().then_some(
                                                                Action::SetSeriesAreaBaseline {
                                                                    series_id: series.id,
                                                                    baseline: Some(baseline),
                                                                },
                                                            ),
                                                            actions,
                                                        );
                                                    });
                                                    if controller.model.axes.y.scale != ScaleType::Linear
                                                        && baseline <= 0.0
                                                    {
                                                        ui.colored_label(
                                                            Color32::RED,
                                                            "Log Y axes require a positive fill baseline",
                                                        );
                                                    }
                                                }
                                                let mut width = series.style.line_width;
                                                let width_response = ui.add(
                                                    egui::Slider::new(&mut width, 1.0..=10.0)
                                                        .text("Width"),
                                                );
                                                queue_continuous_action(
                                                    &width_response,
                                                    width_response.changed().then_some(
                                                        Action::SetSeriesLineWidth {
                                                            series_id: series.id,
                                                            width,
                                                        },
                                                    ),
                                                    actions,
                                                );

                                                let mut style = series.style.line_style;
                                                egui::ComboBox::from_label("Line style")
                                                    .selected_text(line_style_text(style))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(
                                                            &mut style,
                                                            LineStyle::Solid,
                                                            "Solid",
                                                        );
                                                        ui.selectable_value(
                                                            &mut style,
                                                            LineStyle::Dashed,
                                                            "Dashed",
                                                        );
                                                        ui.selectable_value(
                                                            &mut style,
                                                            LineStyle::Dotted,
                                                            "Dotted",
                                                        );
                                                    });
                                                if style != series.style.line_style {
                                                    actions.push(Action::SetSeriesLineStyle {
                                                        series_id: series.id,
                                                        line_style: style,
                                                    });
                                                }

                                                let mut interpolation =
                                                    series.style.line_interpolation;
                                                egui::ComboBox::from_label("Interpolation")
                                                    .selected_text(line_interpolation_text(
                                                        interpolation,
                                                    ))
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(
                                                            &mut interpolation,
                                                            LineInterpolation::Linear,
                                                            "Linear",
                                                        );
                                                        ui.selectable_value(
                                                            &mut interpolation,
                                                            LineInterpolation::StepAfter,
                                                            "Step after",
                                                        );
                                                    });
                                                if interpolation != series.style.line_interpolation
                                                {
                                                    actions.push(
                                                        Action::SetSeriesLineInterpolation {
                                                            series_id: series.id,
                                                            interpolation,
                                                        },
                                                    );
                                                }

                                                let mut color = color32_from_model(series.style.color);
                                                let color_response = color_edit_button(ui, "Series color", &mut color);
                                                queue_color_gesture_begin(ui, &color_response, actions);
                                                if color_response.changed() {
                                                    actions.push(Action::SetSeriesColor {
                                                        series_id: series.id,
                                                        color: model_color_from_ui(color),
                                                    });
                                                }
                                                queue_color_gesture_end(ui, &color_response, actions);

                                                let mut marker_enabled =
                                                    series.style.marker.is_some();
                                                if ui
                                                    .checkbox(&mut marker_enabled, "Marker")
                                                    .changed()
                                                {
                                                    actions.push(Action::SetSeriesMarker {
                                                        series_id: series.id,
                                                        marker: if marker_enabled {
                                                            Some(series.style.marker_preset.shape)
                                                        } else {
                                                            None
                                                        },
                                                        size: series
                                                            .style
                                                            .marker
                                                            .as_ref()
                                                            .map(|m| m.size)
                                                            .unwrap_or(series.style.marker_preset.size),
                                                    });
                                                }
                                                if marker_enabled {
                                                    let mut marker_shape = series
                                                        .style
                                                        .marker
                                                        .as_ref()
                                                        .map(|marker| marker.shape)
                                                        .unwrap_or(series.style.marker_preset.shape);
                                                    egui::ComboBox::from_label("Marker shape")
                                                        .selected_text(marker_shape_text(
                                                            marker_shape,
                                                        ))
                                                        .show_ui(ui, |ui| {
                                                            ui.selectable_value(
                                                                &mut marker_shape,
                                                                MarkerShape::Circle,
                                                                "Circle",
                                                            );
                                                            ui.selectable_value(
                                                                &mut marker_shape,
                                                                MarkerShape::Square,
                                                                "Square",
                                                            );
                                                            ui.selectable_value(
                                                                &mut marker_shape,
                                                                MarkerShape::Triangle,
                                                                "Triangle",
                                                            );
                                                            ui.selectable_value(
                                                                &mut marker_shape,
                                                                MarkerShape::Cross,
                                                                "Cross",
                                                            );
                                                        });
                                                    let mut marker_size = series
                                                        .style
                                                        .marker
                                                        .as_ref()
                                                        .map(|marker| marker.size)
                                                        .unwrap_or(series.style.marker_preset.size);
                                                    let shape_changed = series
                                                        .style
                                                        .marker
                                                        .as_ref()
                                                        .map(|marker| marker.shape != marker_shape)
                                                        .unwrap_or(
                                                            series.style.marker_preset.shape
                                                                != marker_shape,
                                                        );
                                                    let marker_response = ui.add(
                                                        egui::Slider::new(
                                                            &mut marker_size,
                                                            1.0..=16.0,
                                                        )
                                                        .text("Marker size"),
                                                    );
                                                    queue_continuous_action(
                                                        &marker_response,
                                                        (shape_changed || marker_response.changed())
                                                            .then_some(Action::SetSeriesMarker {
                                                                series_id: series.id,
                                                                marker: Some(marker_shape),
                                                                size: marker_size,
                                                            }),
                                                        actions,
                                                    );
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(8.0);
                            }

                            if ui.button("Add series").clicked() {
                                actions.push(Action::AddSeries {
                                    name: String::new(),
                                    x_column: String::new(),
                                    y_column: String::new(),
                                });
                            }
                        });
                    self.draw_export_settings(ui);
                });
            });
    }

    fn draw_export_settings(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.collapsing("Publication export", |ui| {
            ui.checkbox(
                &mut self.match_export_aspect_ratio,
                "Preview at export aspect ratio",
            );
            ui.horizontal_wrapped(|ui| {
                for (label, width, height) in [
                    ("Single column", 85.0, 60.0),
                    ("Double column", 180.0, 120.0),
                    ("Square", 85.0, 85.0),
                    ("Slide 16:9", 180.0, 101.25),
                ] {
                    if ui.small_button(label).clicked() {
                        self.export_dimensions = PublicationDimensions {
                            width,
                            height,
                            unit: PhysicalUnit::Millimeters,
                            dpi: self.export_dimensions.dpi,
                        };
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Width");
                ui.add(
                    egui::DragValue::new(&mut self.export_dimensions.width)
                        .speed(0.5)
                        .range(0.1..=1000.0),
                );
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut self.export_dimensions.height)
                        .speed(0.5)
                        .range(0.1..=1000.0),
                );
                let old_unit = self.export_dimensions.unit;
                egui::ComboBox::from_id_salt("export_physical_unit")
                    .selected_text(match self.export_dimensions.unit {
                        PhysicalUnit::Millimeters => "mm",
                        PhysicalUnit::Inches => "in",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.export_dimensions.unit,
                            PhysicalUnit::Millimeters,
                            "mm",
                        );
                        ui.selectable_value(
                            &mut self.export_dimensions.unit,
                            PhysicalUnit::Inches,
                            "in",
                        );
                    });
                if old_unit != self.export_dimensions.unit {
                    let conversion = match self.export_dimensions.unit {
                        PhysicalUnit::Millimeters => 25.4,
                        PhysicalUnit::Inches => 1.0 / 25.4,
                    };
                    self.export_dimensions.width *= conversion;
                    self.export_dimensions.height *= conversion;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Raster DPI");
                ui.add(
                    egui::DragValue::new(&mut self.export_dimensions.dpi)
                        .range(36..=4800)
                        .speed(25),
                );
            });
            match self.export_dimensions.pixel_size() {
                Ok(pixels) => {
                    let (width_mm, height_mm) = self.export_dimensions.size_in_millimeters();
                    ui.label(format!(
                        "Final: {width_mm:.1} × {height_mm:.1} mm · {} × {} px at {} DPI",
                        pixels.width, pixels.height, self.export_dimensions.dpi
                    ));
                    ui.small(format!(
                        "Fonts stay in pixels; 12 px text prints at {:.1} pt at this DPI. SVG keeps the physical dimensions.",
                        12.0 * 72.0 / self.export_dimensions.dpi as f64
                    ));
                }
                Err(error) => {
                    ui.colored_label(Color32::RED, error);
                }
            }
        });
    }

    fn draw_plot(&mut self, ui: &mut Ui, controller: &PlotController, actions: &mut Vec<Action>) {
        egui::CentralPanel::default().show(ui, |ui| {
            if !controller.has_data() {
                ui.heading("No data loaded");
                ui.label("Use Files > From CSV or Files > From TXT");
                return;
            }

            let available_plot_rect = ui.available_rect_before_wrap();
            let plot_rect = if self.match_export_aspect_ratio
                && self.export_dimensions.width.is_finite()
                && self.export_dimensions.height.is_finite()
                && self.export_dimensions.width > 0.0
                && self.export_dimensions.height > 0.0
            {
                fit_aspect_rect(
                    available_plot_rect,
                    (self.export_dimensions.width / self.export_dimensions.height) as f32,
                )
            } else {
                available_plot_rect
            };
            let preview_scale = self
                .export_dimensions
                .pixel_size()
                .ok()
                .filter(|pixels| pixels.width > 0)
                .map(|pixels| {
                    preview_scale_for_width(
                        plot_rect.width(),
                        ui.ctx().pixels_per_point(),
                        pixels.width,
                    )
                })
                .filter(|scale| scale.is_finite() && *scale > 0.0)
                .unwrap_or(1.0);
            let mut render_model = controller.model.clone();
            scale_plot_model_dimensions(&mut render_model, preview_scale);
            #[cfg(test)]
            {
                self.last_plot_rect = Some(plot_rect);
            }

            let mut rendered = Vec::new();
            let mut series_errors = Vec::new();
            for series in &controller.model.series {
                if !series.visible {
                    continue;
                }
                let result = controller.points_for_series(series.id).and_then(|points| {
                    controller
                        .error_bars_for_series(series.id)
                        .and_then(|error_bars| {
                            controller
                                .confidence_band_segments_for_series(series.id)
                                .map(|bands| (points, error_bars, bands))
                        })
                });
                match result {
                    Ok((points, error_bars, confidence_band_segments)) => {
                        let (scaled, line_segments) = crate::controller::scale_points_into_segments(
                            &points,
                            controller.model.axes.x.scale,
                            controller.model.axes.y.scale,
                        );
                        let area_baseline = series
                            .style
                            .area_fill_visible
                            .then_some(series.style.area_baseline)
                            .flatten()
                            .and_then(|value| {
                                crate::controller::scale_value(value, controller.model.axes.y.scale)
                            });
                        let mut render_series = series.clone();
                        scale_series_dimensions(&mut render_series, preview_scale);
                        rendered.push((
                            render_series,
                            scaled,
                            line_segments,
                            error_bars,
                            area_baseline,
                            confidence_band_segments,
                        ));
                    }
                    Err(error) => series_errors.push(format!("{}: {error}", series.name)),
                }
            }
            for error in &series_errors {
                ui.colored_label(Color32::RED, error);
            }
            let panel_configs = if render_model.panels.is_empty() {
                vec![crate::model::PlotPanel {
                    id: 1,
                    title: String::new(),
                    series_ids: render_model.series.iter().map(|series| series.id).collect(),
                    axis_view: None,
                }]
            } else {
                render_model.panels.clone()
            };
            let data_points = || {
                rendered
                    .iter()
                    .filter(|(series, _, _, _, _, _)| {
                        render_model.panels.is_empty()
                            || render_model
                                .panels
                                .iter()
                                .any(|panel| panel.series_ids.contains(&series.id))
                    })
                    .flat_map(|(_, points, _, errors, area_baseline, bands)| {
                        points.iter().copied().chain(
                            errors
                                .bars
                                .iter()
                                .flat_map(crate::view::series::ErrorBar::range_points)
                                .chain(area_baseline.iter().copied().flat_map(|baseline| {
                                    points.iter().map(move |(x, _)| (*x, baseline))
                                })),
                        )
                        .chain(bands.iter().flat_map(|segment| segment.iter().flat_map(|(x, low, high)| [(*x, *low), (*x, *high)])))
                    })
            };
            let x_range = resolve_data_range(
                &render_model.axes.x.range,
                data_points(),
                true,
                -1.0..1.0,
                render_model.axes.x.scale,
            );
            let y_range = resolve_data_range(
                &render_model.axes.y.range,
                data_points(),
                false,
                -1.0..1.0,
                render_model.axes.y.scale,
            );
            let mut panel_states = Vec::with_capacity(panel_configs.len());
            for panel in &panel_configs {
                match build_preview_panel(
                    controller,
                    panel,
                    preview_scale,
                    x_range.clone(),
                    y_range.clone(),
                ) {
                    Ok(state) => panel_states.push(state),
                    Err(error) => {
                        ui.colored_label(Color32::RED, error);
                        panel_states.push(PreviewPanelData {
                            panel: panel.clone(),
                            axes: controller
                                .effective_panel_axis_view(panel.id)
                                .unwrap_or(PanelAxisView {
                                    x_scale: render_model.axes.x.scale,
                                    y_scale: render_model.axes.y.scale,
                                    x_range: render_model.axes.x.range.clone(),
                                    y_range: render_model.axes.y.range.clone(),
                                }),
                            x_range: x_range.clone(),
                            y_range: y_range.clone(),
                            series: Vec::new(),
                        });
                    }
                }
            }
            let omitted_log_intervals = panel_states
                .iter()
                .flat_map(|state| {
                    state.series.iter().filter_map(move |data| {
                        let x = data.error_bars.omitted_log_x;
                        let y = data.error_bars.omitted_log_y;
                        (x > 0 || y > 0).then(|| {
                            (state.panel.title.clone(), data.series.name.clone(), x, y)
                        })
                    })
                })
                .collect::<Vec<_>>();

            let title = if render_model.layout.title_visible {
                render_model.layout.title.trim().to_owned()
            } else {
                String::new()
            };
            let panel_mode = !render_model.panels.is_empty();
            let has_chart_caption = panel_mode || !title.is_empty();
            let marker_title_clearance = rendered
                .iter()
                .filter(|(series, _, _, _, _, _)| {
                    render_model.panels.is_empty()
                        || render_model
                            .panels
                            .iter()
                            .any(|panel| panel.series_ids.contains(&series.id))
                })
                .filter_map(|(series, _, _, _, _, _)| {
                    series
                        .style
                        .marker
                        .as_ref()
                        .map(|marker| marker.size.ceil().max(0.0) as u32 * 2)
                })
                .max()
                .unwrap_or_default();
            let y_ticks = render_model.axes.y.ticks.clone();
            let x_axis_title_font_size = render_model.axes.x.axis_title_font_size;
            let y_axis_title_font_size = render_model.axes.y.axis_title_font_size;
            let x_label_font_size = render_model.axes.x.label_font_size;
            let y_label_font_size = render_model.axes.y.label_font_size;
            let legend_visible = render_model.legend.visible;
            let margin = render_model.layout.margin;
            let x_label_area = render_model.layout.x_label_area_size;
            let y_label_area = render_model.layout.y_label_area_size;
            let font_family = render_model.layout.font_family.plotters_name();
            let (mut effective_x_label_area, mut effective_y_label_area) = resolve_label_areas(
                &y_range,
                &y_ticks,
                render_model.axes.y.scale,
                &font_family,
                &render_model.axes.x.label,
                &render_model.axes.y.label,
                x_label_area,
                y_label_area,
                x_label_font_size,
                y_label_font_size,
                x_axis_title_font_size,
                y_axis_title_font_size,
            );
            for state in &panel_states {
                let (x_area, y_area) = resolve_label_areas(
                    &state.y_range,
                    &y_ticks,
                    state.axes.y_scale,
                    &font_family,
                    &render_model.axes.x.label,
                    &render_model.axes.y.label,
                    x_label_area,
                    y_label_area,
                    x_label_font_size,
                    y_label_font_size,
                    x_axis_title_font_size,
                    y_axis_title_font_size,
                );
                effective_x_label_area = effective_x_label_area.max(x_area);
                effective_y_label_area = effective_y_label_area.max(y_area);
            }
            let title_font_size = render_model.layout.title_font_size;
            let chart_caption_padding = if has_chart_caption {
                title_font_size.saturating_add(marker_title_clearance)
            } else {
                0
            };
            let title_font_color = RGBAColor(
                render_model.layout.title_font_color.r,
                render_model.layout.title_font_color.g,
                render_model.layout.title_font_color.b,
                render_model.layout.title_font_color.a as f64 / 255.0,
            );
            let legend_series = panel_legend_series(&panel_states);
            let legend_names = legend_series
                .iter()
                .map(|data| data.series.name.as_str())
                .collect::<Vec<_>>();
            let dragged_legend_anchor_id =
                ui.make_persistent_id("plot-legend").with("dragged-anchor");
            let transient_legend_anchor =
                ui.data(|data| data.get_temp::<[f32; 2]>(dragged_legend_anchor_id));
            let mut legend_render_config = render_model.legend.clone();
            if let Some(anchor) = transient_legend_anchor {
                legend_render_config.anchor = anchor;
                legend_render_config.position = LegendPosition::Custom;
            }
            let chart_margins = chart_margins_for_legend(
                margin,
                &legend_render_config,
                &legend_names,
                chart_caption_padding,
                effective_x_label_area,
            );
            let x_title_anchor_id = ui
                .make_persistent_id(("axis-title", false))
                .with("dragged-anchor");
            let y_title_anchor_id = ui
                .make_persistent_id(("axis-title", true))
                .with("dragged-anchor");
            let transient_x_title_anchor =
                ui.data(|data| data.get_temp::<[f32; 2]>(x_title_anchor_id));
            let transient_y_title_anchor =
                ui.data(|data| data.get_temp::<[f32; 2]>(y_title_anchor_id));
            let mut render_config = ChartRenderConfig::from_model(
                &render_model,
                x_range.clone(),
                y_range.clone(),
                chart_margins,
                chart_caption_padding,
                effective_x_label_area,
                effective_y_label_area,
            );
            render_config.x_title_anchor =
                transient_x_title_anchor.or(render_config.x_title_anchor);
            render_config.y_title_anchor =
                transient_y_title_anchor.or(render_config.y_title_anchor);

            let legend_title = render_model.legend.title.clone();
            let draw_legend = render_model.legend.visible;
            let render_error = Rc::new(RefCell::new(None::<String>));
            let callback_error = Rc::clone(&render_error);
            let draw_panel_states = panel_states.clone();
            let draw_render_config = render_config.clone();
            let panel_columns = (panel_configs.len() as f32).sqrt().ceil() as usize;
            let panel_rows = panel_configs.len().div_ceil(panel_columns.max(1));
            let panel_count = panel_configs.len();
            let interactive_panel_series_ids = panel_configs
                .iter()
                .map(|panel| panel.series_ids.clone())
                .collect::<Vec<_>>();
            let mut chart = Chart::new((x_range.clone(), y_range.clone()))
                .mouse(MouseConfig::default())
                .builder_cb(Box::new(move |area, _t, _ranges| {
                    let cells = area.split_evenly((panel_rows, panel_columns.max(1)));
                    for (panel_index, (panel, cell)) in
                        panel_configs.iter().zip(cells.iter()).enumerate()
                    {
                        let panel_state = &draw_panel_states[panel_index];
                        let panel_render_config = draw_render_config.for_panel(
                            &panel_state.axes,
                            panel_state.x_range.clone(),
                            panel_state.y_range.clone(),
                        );
                        let panel_title = if panel_mode {
                            if panel_index == 0 && !title.trim().is_empty() {
                                format!("{} — {}", title, panel.title)
                            } else {
                                panel.title.clone()
                            }
                        } else {
                            title.clone()
                        };
                        let mut builder = ChartBuilder::on(cell);
                        if !panel_title.trim().is_empty() {
                            builder.caption(
                                panel_title,
                                (font_family.as_str(), title_font_size)
                                    .into_font()
                                    .color(&title_font_color),
                            );
                        }
                        let mut panel_chart = match builder
                            .margin_top(chart_margins.top)
                            .margin_bottom(chart_margins.bottom)
                            .margin_left(chart_margins.left)
                            .margin_right(chart_margins.right)
                            .x_label_area_size(effective_x_label_area)
                            .y_label_area_size(effective_y_label_area)
                            .build_cartesian_2d(
                                panel_state.x_range.clone(),
                                panel_state.y_range.clone(),
                            )
                        {
                            Ok(chart) => chart,
                            Err(error) => {
                                *callback_error.borrow_mut() = Some(error.to_string());
                                return;
                            }
                        };

                        let panel_series = &panel_state.series;
                        let series_to_draw = panel_series
                            .iter()
                            .map(|data| {
                                SeriesRender {
                                    points: &data.points,
                                    line_segments: &data.line_segments,
                                    confidence_band_segments: &data.confidence_band_segments,
                                    error_bars: &data.error_bars.bars,
                                    area_baseline: data.area_baseline,
                                    color: data.series.style.color,
                                    line_visible: data.series.style.line_visible,
                                    line_width: data.series.style.line_width,
                                    line_style: data.series.style.line_style,
                                    line_interpolation: data.series.style.line_interpolation,
                                    marker: data.series.style.marker.as_ref(),
                                }
                            })
                            .collect::<Vec<_>>();
                        if let Err(error) = draw_chart_content(
                            cell,
                            &mut panel_chart,
                            &panel_render_config,
                            &series_to_draw,
                        ) {
                            *callback_error.borrow_mut() = Some(error.to_string());
                            return;
                        }
                        if draw_legend && !panel_mode {
                            let legend_items = panel_series
                                .iter()
                                .filter(|data| !data.points.is_empty())
                                .map(|data| LegendItem {
                                    name: &data.series.name,
                                    color: data.series.style.color,
                                    line_visible: data.series.style.line_visible,
                                    line_style: data.series.style.line_style,
                                    line_interpolation: data.series.style.line_interpolation,
                                    area_fill: data.series.style.area_fill_visible || data.series.confidence_band.is_some(),
                                    marker: data.series.style.marker.as_ref(),
                                })
                                .collect::<Vec<_>>();
                            if let Err(error) = draw_custom_legend(
                                cell,
                                legend_title.as_deref(),
                                &legend_items,
                                &legend_render_config,
                                &font_family,
                                margin,
                                chart_caption_padding,
                            ) {
                                *callback_error.borrow_mut() = Some(error.to_string());
                                return;
                            }
                        }
                    }
                    if draw_legend && panel_mode {
                        let legend_series = panel_legend_series(&draw_panel_states);
                        let legend_items = legend_series
                            .iter()
                            .map(|data| LegendItem {
                                name: &data.series.name,
                                color: data.series.style.color,
                                line_visible: data.series.style.line_visible,
                                line_style: data.series.style.line_style,
                                line_interpolation: data.series.style.line_interpolation,
                                area_fill: data.series.style.area_fill_visible || data.series.confidence_band.is_some(),
                                marker: data.series.style.marker.as_ref(),
                            })
                            .collect::<Vec<_>>();
                        if let Err(error) = draw_custom_legend(
                            area,
                            legend_title.as_deref(),
                            &legend_items,
                            &legend_render_config,
                            &font_family,
                            margin,
                            chart_caption_padding,
                        ) {
                            *callback_error.borrow_mut() = Some(error.to_string());
                            return;
                        }
                    }
                }));

            chart.draw_in_rect(ui, plot_rect);
            for (panel_name, series_name, omitted_x, omitted_y) in omitted_log_intervals {
                    let mut omitted = Vec::new();
                    if omitted_x > 0 {
                        omitted.push(format!("X: {omitted_x}"));
                    }
                    if omitted_y > 0 {
                        omitted.push(format!("Y: {omitted_y}"));
                    }
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "{}{}: log scale omits error intervals ({}) because an endpoint is non-positive",
                            if panel_name.is_empty() { String::new() } else { format!("{} / ", panel_name) },
                            series_name,
                            omitted.join(", ")
                        ),
                    );
            }
            let panel_canvas_rects = split_plot_canvas(plot_rect, panel_count);
            let panel_data_rects = panel_canvas_rects
                .iter()
                .map(|cell| {
                    panel_data_rect(
                        *cell,
                        chart_margins,
                        chart_caption_padding,
                        effective_x_label_area,
                        effective_y_label_area,
                    )
                })
                .collect::<Vec<_>>();
            #[cfg(test)]
            {
                self.last_data_rect = Some(panel_data_rects[0]);
                self.last_panel_data_rects = panel_data_rects.clone();
            }
            let navigation = ui.interact(
                plot_rect,
                ui.make_persistent_id("plot-data-navigation"),
                egui::Sense::click_and_drag(),
            );
            let pointer_excluded = |position: egui::Pos2| {
                panel_canvas_rects.iter().any(|panel_canvas| {
                    axis_title_hitboxes(
                        *panel_canvas,
                        &render_model,
                        effective_x_label_area,
                        effective_y_label_area,
                        chart_caption_padding,
                        chart_margins,
                        transient_x_title_anchor,
                        transient_y_title_anchor,
                    )
                    .iter()
                    .any(|(_, rect)| rect.contains(position))
                })
                    || (legend_visible
                        && legend_hitbox(
                            ui,
                            plot_rect,
                            controller,
                            &render_model,
                            margin,
                            chart_caption_padding,
                        )
                            .is_some_and(|rect| rect.contains(position)))
            };
            if navigation.clicked_by(egui::PointerButton::Primary)
                && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                && let Some(panel_index) = panel_data_rects.iter().position(|rect| rect.contains(pointer))
                && !pointer_excluded(pointer)
            {
                let hit_rect = panel_data_rects[panel_index];
                let active_panel = &panel_states[panel_index];
                let assigned_series = &interactive_panel_series_ids[panel_index];
                let mut nearest_series: Option<(f32, crate::model::SeriesId)> = None;
                let mut nearest_point: Option<(f32, crate::model::SeriesId, usize)> = None;
                for series in controller
                    .model
                    .series
                    .iter()
                    .filter(|series| series.visible && assigned_series.contains(&series.id))
                {
                    let Ok(points) = controller.points_for_series(series.id) else {
                        continue;
                    };
                    let mut previous: Option<egui::Pos2> = None;
                    for (row_index, (x, y)) in points.into_iter().enumerate() {
                        let (Some(x), Some(y)) = (
                            crate::controller::scale_value(x, active_panel.axes.x_scale),
                            crate::controller::scale_value(y, active_panel.axes.y_scale),
                        ) else {
                            previous = None;
                            continue;
                        };
                        let point_position = egui::pos2(
                            hit_rect.left()
                                + ((x - active_panel.x_range.start)
                                    / (active_panel.x_range.end - active_panel.x_range.start)) as f32
                                    * hit_rect.width(),
                            hit_rect.bottom()
                                - ((y - active_panel.y_range.start)
                                    / (active_panel.y_range.end - active_panel.y_range.start)) as f32
                                    * hit_rect.height(),
                        );
                        let point_in_view = range_contains_inclusive(&active_panel.x_range, x)
                            && range_contains_inclusive(&active_panel.y_range, y);
                        if point_in_view {
                            let distance = pointer.distance(point_position);
                            if nearest_series.is_none_or(|(best, _)| distance < best) {
                                nearest_series = Some((distance, series.id));
                            }
                            if nearest_point.is_none_or(|(best, _, _)| distance < best) {
                                nearest_point = Some((distance, series.id, row_index));
                            }
                        }
                        if series.style.line_visible
                            && let Some(previous_position) = previous
                        {
                            let line_points = if series.style.line_interpolation
                                == LineInterpolation::StepAfter
                            {
                                [
                                    (previous_position.x, previous_position.y),
                                    (point_position.x, previous_position.y),
                                    (point_position.x, point_position.y),
                                ]
                            } else {
                                [
                                    (previous_position.x, previous_position.y),
                                    (point_position.x, point_position.y),
                                    (point_position.x, point_position.y),
                                ]
                            };
                            for pair in line_points.windows(2) {
                                if let Some((start, end)) = clip_segment_to_rect(
                                    egui::pos2(pair[0].0, pair[0].1),
                                    egui::pos2(pair[1].0, pair[1].1),
                                    hit_rect,
                                ) {
                                    let distance = distance_to_segment(pointer, start, end);
                                    if nearest_series.is_none_or(|(best, _)| distance < best) {
                                        nearest_series = Some((distance, series.id));
                                    }
                                }
                            }
                        }
                        previous = Some(point_position);
                    }
                }
                self.selected_series = nearest_series
                    .filter(|(distance, _)| *distance <= 14.0)
                    .map(|(_, series_id)| series_id);
                self.selected_data_revision =
                    self.selected_series.map(|_| controller.data_revision());
                self.inspected_point = nearest_point
                    .filter(|(distance, _, _)| *distance <= 14.0)
                    .map(|(_, series_id, row_index)| (series_id, row_index));
            }
            let (pointer_position, pointer_delta, middle_down, scroll_y) = ui.input(|input| {
                (
                    input.pointer.hover_pos(),
                    input.pointer.delta(),
                    input.pointer.button_down(egui::PointerButton::Middle),
                    input
                        .raw
                        .events
                        .iter()
                        .filter_map(|event| match event {
                            egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                            _ => None,
                        })
                        .sum::<f32>(),
                )
            });
            if let Some(pointer) = pointer_position
                .filter(|position| panel_data_rects.iter().any(|rect| rect.contains(*position)) && !pointer_excluded(*position))
            {
                let panel_index = panel_data_rects.iter().position(|rect| rect.contains(pointer)).unwrap_or_default();
                let active_rect = panel_data_rects[panel_index];
                let active_panel = &panel_states[panel_index];
                let dragging = navigation.dragged_by(egui::PointerButton::Primary) || middle_down;
                let mut next_x = active_panel.x_range.clone();
                let mut next_y = active_panel.y_range.clone();
                let dragging = dragging && pointer_delta.length_sq() > 0.0;
                if dragging {
                    next_x = pan_range(&next_x, -pointer_delta.x, active_rect.width())
                        .unwrap_or_else(|| next_x.clone());
                    next_y = pan_range(&next_y, pointer_delta.y, active_rect.height())
                        .unwrap_or_else(|| next_y.clone());
                }
                if scroll_y.abs() > 0.0 {
                    let factor = (scroll_y as f64 * 0.002).exp().clamp(0.05, 20.0);
                    let x_anchor = ((pointer.x - active_rect.left()) / active_rect.width()).clamp(0.0, 1.0);
                    let y_anchor = ((active_rect.bottom() - pointer.y) / active_rect.height()).clamp(0.0, 1.0);
                    next_x = zoom_range(&next_x, x_anchor as f64, factor)
                        .unwrap_or_else(|| next_x.clone());
                    next_y = zoom_range(&next_y, y_anchor as f64, factor)
                        .unwrap_or_else(|| next_y.clone());
                }
                if (dragging || scroll_y.abs() > 0.0)
                    && let (Some(x), Some(y)) = (
                        to_data_range(next_x, active_panel.axes.x_scale),
                        to_data_range(next_y, active_panel.axes.y_scale),
                    )
                {
                    if active_panel.panel.axis_view.is_some() {
                        actions.push(Action::SetPanelAxisRanges {
                            panel_id: active_panel.panel.id,
                            x,
                            y,
                        });
                    } else {
                        actions.push(Action::SetAxesRanges { x, y });
                    }
                }
            }
            if navigation.drag_stopped()
                || ui.input(|input| input.pointer.button_released(egui::PointerButton::Middle))
                || scroll_y.abs() > 0.0
            {
                actions.push(Action::EndAxesGesture);
            }
            if let Some(error) = render_error.borrow().as_ref() {
                ui.painter().text(
                    plot_rect.left_top() + egui::vec2(8.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    format!("Plot rendering failed: {error}"),
                    egui::FontId::proportional(14.0),
                    Color32::RED,
                );
            }
            let axis_title_rects = interact_axis_title_drags(
                ui,
                panel_canvas_rects[0],
                &render_model,
                actions,
                effective_x_label_area,
                effective_y_label_area,
                chart_caption_padding,
                chart_margins,
                transient_x_title_anchor,
                transient_y_title_anchor,
            );
            #[cfg(test)]
            {
                self.last_axis_title_hitboxes = Some(axis_title_rects);
            }
            #[cfg(not(test))]
            let _ = axis_title_rects;
            if legend_visible {
                let legend_rect = draw_interactive_legend(
                    ui,
                    plot_rect,
                    controller,
                    &render_model,
                    actions,
                    margin,
                    chart_caption_padding,
                );
                #[cfg(test)]
                {
                    self.last_legend_rect = legend_rect;
                }
                #[cfg(not(test))]
                let _ = legend_rect;
            }
        });
    }
}

impl Default for PlotEditorView {
    fn default() -> Self {
        Self::new()
    }
}

fn pan_range(
    range: &std::ops::Range<f64>,
    delta_pixels: f32,
    extent_pixels: f32,
) -> Option<std::ops::Range<f64>> {
    let span = range.end - range.start;
    if !span.is_finite() || span <= 0.0 || !extent_pixels.is_finite() || extent_pixels <= 0.0 {
        return None;
    }
    let shift = delta_pixels as f64 / extent_pixels as f64 * span;
    let next = (range.start + shift)..(range.end + shift);
    (next.start.is_finite() && next.end.is_finite() && next.start < next.end).then_some(next)
}

fn zoom_range(
    range: &std::ops::Range<f64>,
    anchor_fraction: f64,
    factor: f64,
) -> Option<std::ops::Range<f64>> {
    let span = range.end - range.start;
    if !span.is_finite() || span <= 0.0 || !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let anchor = range.start + span * anchor_fraction.clamp(0.0, 1.0);
    let next_span = (span * factor).clamp(f64::MIN_POSITIVE, f64::MAX / 4.0);
    let min = anchor - next_span * anchor_fraction.clamp(0.0, 1.0);
    let max = min + next_span;
    (min.is_finite() && max.is_finite() && min < max).then_some(min..max)
}

fn to_data_range(range: std::ops::Range<f64>, scale: ScaleType) -> Option<RangePolicy> {
    let inverse = |value: f64| match scale {
        ScaleType::Linear => Some(value),
        ScaleType::Log10 | ScaleType::Log10Axis => Some(10.0_f64.powf(value)),
        ScaleType::LogE => Some(value.exp()),
    };
    let (min, max) = (inverse(range.start)?, inverse(range.end)?);
    if min.is_finite() && max.is_finite() && min < max && (scale == ScaleType::Linear || min > 0.0)
    {
        Some(RangePolicy::Manual { min, max })
    } else {
        None
    }
}

fn axis_title_hitboxes(
    canvas: egui::Rect,
    model: &PlotModel,
    x_area: u32,
    y_area: u32,
    caption_height: u32,
    margins: ChartMargins,
    transient_x_anchor: Option<[f32; 2]>,
    transient_y_anchor: Option<[f32; 2]>,
) -> [(AxisKind, egui::Rect); 2] {
    let x_label = format!(
        "{}{}",
        model.axes.x.label,
        crate::view::series::scale_suffix(model.axes.x.scale)
    );
    let y_label = format!(
        "{}{}",
        model.axes.y.label,
        crate::view::series::scale_suffix(model.axes.y.scale)
    );
    let geometry = axis_title_geometry(
        (
            canvas.width().round() as u32,
            canvas.height().round() as u32,
        ),
        &x_label,
        &y_label,
        model.axes.x.axis_title_font_size,
        model.axes.y.axis_title_font_size,
        x_area,
        y_area,
        caption_height,
        margins,
        &model.layout.font_family.plotters_name(),
        transient_x_anchor.or(model.axes.x.title_anchor),
        transient_y_anchor.or(model.axes.y.title_anchor),
    );
    let x_center = canvas.min + egui::vec2(geometry.x_center.0, geometry.x_center.1);
    let y_center = canvas.min + egui::vec2(geometry.y_center.0, geometry.y_center.1);
    [
        (
            AxisKind::X,
            egui::Rect::from_center_size(
                x_center,
                egui::vec2(geometry.x_size.0, geometry.x_size.1),
            ),
        ),
        (
            AxisKind::Y,
            egui::Rect::from_center_size(
                y_center,
                egui::vec2(geometry.y_size.0, geometry.y_size.1),
            ),
        ),
    ]
}

fn split_plot_canvas(canvas: egui::Rect, panel_count: usize) -> Vec<egui::Rect> {
    let count = panel_count.max(1);
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns.max(1));
    let cell_width = canvas.width() / columns as f32;
    let cell_height = canvas.height() / rows as f32;
    (0..count)
        .map(|index| {
            let column = index % columns;
            let row = index / columns;
            let min = canvas.min + egui::vec2(column as f32 * cell_width, row as f32 * cell_height);
            egui::Rect::from_min_size(min, egui::vec2(cell_width, cell_height))
        })
        .collect()
}

fn panel_data_rect(
    canvas: egui::Rect,
    margins: ChartMargins,
    caption_height: u32,
    x_label_area: u32,
    y_label_area: u32,
) -> egui::Rect {
    let left = margins.left as f32 + y_label_area as f32;
    let top = margins.top as f32 + caption_height as f32;
    let width = (canvas.width() - left - margins.right as f32).max(1.0);
    let height = (canvas.height() - top - margins.bottom as f32 - x_label_area as f32).max(1.0);
    egui::Rect::from_min_size(
        canvas.min + egui::vec2(left, top),
        egui::vec2(width, height),
    )
}

fn interact_axis_title_drags(
    ui: &mut Ui,
    canvas: egui::Rect,
    model: &PlotModel,
    actions: &mut Vec<Action>,
    x_area: u32,
    y_area: u32,
    caption_height: u32,
    margins: ChartMargins,
    transient_x_anchor: Option<[f32; 2]>,
    transient_y_anchor: Option<[f32; 2]>,
) -> [(AxisKind, egui::Rect); 2] {
    let hitboxes = axis_title_hitboxes(
        canvas,
        model,
        x_area,
        y_area,
        caption_height,
        margins,
        transient_x_anchor,
        transient_y_anchor,
    );
    for (axis, rect) in hitboxes {
        let id = ui.make_persistent_id(("axis-title", matches!(axis, AxisKind::Y)));
        let drag_offset_id = id.with("drag-offset");
        let dragged_anchor_id = id.with("dragged-anchor");
        let response = ui.interact(rect, id, egui::Sense::drag());
        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            ui.data_mut(|data| data.insert_temp(drag_offset_id, pointer - rect.center()));
        }
        if response.dragged()
            && let (Some(pointer), Some(offset)) = (
                response.interact_pointer_pos(),
                ui.data(|data| data.get_temp::<egui::Vec2>(drag_offset_id)),
            )
        {
            let center = pointer - offset;
            let anchor = [
                ((center.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0),
                ((center.y - canvas.top()) / canvas.height()).clamp(0.0, 1.0),
            ];
            ui.data_mut(|data| data.insert_temp(dragged_anchor_id, anchor));
        }
        if response.drag_stopped() {
            if let Some(anchor) = ui.data(|data| data.get_temp::<[f32; 2]>(dragged_anchor_id)) {
                actions.push(Action::SetAxisTitleAnchor {
                    axis,
                    anchor: Some(anchor),
                });
            }
            ui.data_mut(|data| {
                data.remove::<egui::Vec2>(drag_offset_id);
                data.remove::<[f32; 2]>(dragged_anchor_id);
            });
        }
    }
    hitboxes
}

fn legend_hitbox(
    ui: &Ui,
    canvas: egui::Rect,
    controller: &PlotController,
    model: &PlotModel,
    outer_margin: u32,
    caption_height: u32,
) -> Option<egui::Rect> {
    let config = &model.legend;
    let labels = controller
        .model
        .series
        .iter()
        .filter(|series| series.visible)
        .filter(|series| {
            model.panels.is_empty()
                || model
                    .panels
                    .iter()
                    .any(|panel| panel.series_ids.contains(&series.id))
        })
        .filter(|series| {
            controller
                .points_for_series(series.id)
                .is_ok_and(|points| !points.is_empty())
        })
        .map(|series| series.name.as_str())
        .collect::<Vec<_>>();
    if labels.is_empty() {
        return None;
    }
    let layout = legend_layout(
        config.title.as_deref(),
        &labels,
        config.font_size,
        config.padding,
        config.row_spacing,
        config.sample_length,
        config.columns,
        config.column_spacing,
    );
    let id = ui.make_persistent_id("plot-legend").with("dragged-anchor");
    let transient_anchor = ui.data(|data| data.get_temp::<[f32; 2]>(id));
    let mut effective_config = config.clone();
    if let Some(anchor) = transient_anchor {
        effective_config.anchor = anchor;
        effective_config.position = LegendPosition::Custom;
    }
    let anchor = effective_config.anchor;
    let size = egui::vec2(layout.width, layout.height);
    let position = legend_position(
        canvas,
        &effective_config,
        size,
        anchor,
        outer_margin,
        caption_height,
    );
    Some(egui::Rect::from_min_size(position, size))
}

fn legend_position(
    canvas: egui::Rect,
    config: &crate::model::LegendConfig,
    size: egui::Vec2,
    anchor: [f32; 2],
    outer_margin: u32,
    caption_height: u32,
) -> egui::Pos2 {
    match config.position {
        LegendPosition::OutsideLeft => egui::pos2(
            canvas.left() + outer_margin as f32,
            canvas.center().y - size.y / 2.0,
        ),
        LegendPosition::OutsideRight => egui::pos2(
            canvas.right() - outer_margin as f32 - size.x,
            canvas.center().y - size.y / 2.0,
        ),
        LegendPosition::OutsideTop => egui::pos2(
            canvas.center().x - size.x / 2.0,
            canvas.top()
                + outer_margin as f32
                + caption_height as f32
                + if caption_height > 0 { 10.0 } else { 0.0 },
        ),
        LegendPosition::OutsideBottom => egui::pos2(
            canvas.center().x - size.x / 2.0,
            canvas.bottom() - outer_margin as f32 - size.y,
        ),
        _ => {
            let max_offset = (canvas.size() - size).max(egui::Vec2::ZERO);
            canvas.min + egui::vec2(anchor[0] * max_offset.x, anchor[1] * max_offset.y)
        }
    }
}

fn draw_interactive_legend(
    ui: &mut Ui,
    canvas: egui::Rect,
    controller: &PlotController,
    model: &PlotModel,
    actions: &mut Vec<Action>,
    outer_margin: u32,
    caption_height: u32,
) -> Option<egui::Rect> {
    let Some(original_rect) =
        legend_hitbox(ui, canvas, controller, model, outer_margin, caption_height)
    else {
        return None;
    };
    let anchor_id = ui.make_persistent_id("plot-legend");
    let drag_offset_id = anchor_id.with("drag-offset");
    let dragged_anchor_id = anchor_id.with("dragged-anchor");
    let response = ui.interact(original_rect, anchor_id, egui::Sense::drag());

    if response.drag_started() {
        if let Some(pointer) = response.interact_pointer_pos() {
            ui.data_mut(|data| data.insert_temp(drag_offset_id, pointer - original_rect.min));
        }
    }
    if response.dragged() {
        if let (Some(pointer), Some(offset)) = (
            response.interact_pointer_pos(),
            ui.data(|data| data.get_temp::<egui::Vec2>(drag_offset_id)),
        ) {
            let moved_min = pointer - offset;
            let max_offset = (canvas.size() - original_rect.size()).max(egui::Vec2::ZERO);
            let anchor = [
                ((moved_min.x - canvas.left()) / max_offset.x.max(1.0)).clamp(0.0, 1.0),
                ((moved_min.y - canvas.top()) / max_offset.y.max(1.0)).clamp(0.0, 1.0),
            ];
            ui.data_mut(|data| data.insert_temp(dragged_anchor_id, anchor));
        }
    }
    if response.drag_stopped() {
        if let Some(anchor) = ui.data(|data| data.get_temp::<[f32; 2]>(dragged_anchor_id)) {
            actions.push(Action::SetLegendAnchor(anchor));
        }
        ui.data_mut(|data| {
            data.remove::<egui::Vec2>(drag_offset_id);
            data.remove::<[f32; 2]>(dragged_anchor_id);
        });
    }
    Some(original_rect)
}

fn axis_editor(
    ui: &mut egui::Ui,
    title: &str,
    axis: AxisKind,
    controller: &PlotController,
    text_drafts: &mut HashMap<egui::Id, String>,
    actions: &mut Vec<Action>,
) {
    ui.push_id(title, |ui| {
        ui.collapsing(title, |ui| {
            let axis_ref = match axis {
                AxisKind::X => &controller.model.axes.x,
                AxisKind::Y => &controller.model.axes.y,
            };

            let label_id = ui.make_persistent_id("axis_label");
            if let Some(label) =
                deferred_text_edit(ui, text_drafts, label_id, "Axis label", &axis_ref.label)
            {
                actions.push(Action::SetAxisLabel { axis, label });
            }
            ui.horizontal(|ui| {
                ui.label("Label font");
                let mut label_font = axis_ref.axis_title_font_size;
                let slider_response =
                    ui.add(egui::Slider::new(&mut label_font, 8..=200).show_value(false));
                let input_response = ui.add(egui::DragValue::new(&mut label_font).range(8..=200));
                let changed = slider_response.changed() || input_response.changed();
                let action = (changed && label_font != axis_ref.axis_title_font_size).then_some(
                    Action::SetAxisTitleFontSize {
                        axis,
                        font_size: label_font,
                    },
                );
                queue_gesture_action(&[&slider_response, &input_response], action, actions);
            });

            ui.horizontal(|ui| {
                ui.label("Tick font");
                let mut tick_font = axis_ref.label_font_size;
                let slider_response =
                    ui.add(egui::Slider::new(&mut tick_font, 8..=200).show_value(false));
                let input_response = ui.add(egui::DragValue::new(&mut tick_font).range(8..=200));
                let changed = slider_response.changed() || input_response.changed();
                let action = (changed && tick_font != axis_ref.label_font_size).then_some(
                    Action::SetAxisLabelFontSize {
                        axis,
                        font_size: tick_font,
                    },
                );
                queue_gesture_action(&[&slider_response, &input_response], action, actions);
            });

            let mut manual_title_position = axis_ref.title_anchor.is_some();
            if ui
                .checkbox(&mut manual_title_position, "Manual title position")
                .changed()
            {
                let anchor = if manual_title_position {
                    Some(match axis {
                        AxisKind::X => [0.5, 0.95],
                        AxisKind::Y => [0.05, 0.5],
                    })
                } else {
                    None
                };
                actions.push(Action::SetAxisTitleAnchor { axis, anchor });
            }
            if let Some(anchor) = axis_ref.title_anchor {
                let mut anchor = anchor;
                let (x_response, y_response) = ui
                    .horizontal(|ui| {
                        ui.label("Title X");
                        let x_response =
                            ui.add(egui::Slider::new(&mut anchor[0], 0.0..=1.0));
                        ui.label("Y");
                        let y_response =
                            ui.add(egui::Slider::new(&mut anchor[1], 0.0..=1.0));
                        (x_response, y_response)
                    })
                    .inner;
                let changed = x_response.changed() || y_response.changed();
                queue_gesture_action(
                    &[&x_response, &y_response],
                    changed.then_some(Action::SetAxisTitleAnchor {
                        axis,
                        anchor: Some(anchor),
                    }),
                    actions,
                );
            }

            let mut scale = axis_ref.scale;
            egui::ComboBox::from_label("Scale")
                .selected_text(scale_text(scale))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut scale, ScaleType::Linear, "Linear");
                    ui.selectable_value(&mut scale, ScaleType::Log10Axis, "Log10 axis (1, 10, 100)");
                    ui.selectable_value(&mut scale, ScaleType::Log10, "Plot log10(data)");
                    ui.selectable_value(&mut scale, ScaleType::LogE, "Natural log of data");
                });
            if scale != axis_ref.scale {
                actions.push(Action::SetAxisScale { axis, scale });
            }
            if axis_ref.scale != ScaleType::Linear {
                ui.small("Data values and manual bounds use original units; plotted coordinates are transformed.");
            }

            let mut auto = matches!(axis_ref.range, RangePolicy::Auto);
            ui.horizontal(|ui| {
                if ui.radio_value(&mut auto, true, "Auto").clicked() {
                    actions.push(Action::SetAxisRange {
                        axis,
                        range: RangePolicy::Auto,
                    });
                }
                if ui.radio_value(&mut auto, false, "Manual").clicked()
                    && !matches!(axis_ref.range, RangePolicy::Manual { .. })
                {
                    let fallback = if axis_ref.scale == ScaleType::Linear {
                        (-1.0, 1.0)
                    } else {
                        (1.0, 10.0)
                    };
                    let (min, max) = controller.suggested_axis_range(axis).unwrap_or(fallback);
                    actions.push(Action::SetAxisRange {
                        axis,
                        range: RangePolicy::Manual { min, max },
                    });
                }
            });

            let (mut min, mut max) = match axis_ref.range {
                RangePolicy::Auto => controller
                    .suggested_axis_range(axis)
                    .unwrap_or_else(|| match axis_ref.scale {
                        ScaleType::Linear => (-1.0, 1.0),
                        _ => (1.0, 10.0),
                    }),
                RangePolicy::Manual { min, max } => (min, max),
            };
            ui.horizontal(|ui| {
                ui.label(if axis_ref.scale == ScaleType::Linear {
                    "Min"
                } else {
                    "Min (data)"
                });
                let min_response =
                    ui.add_enabled(!auto, egui::DragValue::new(&mut min).speed(0.1));
                ui.label(if axis_ref.scale == ScaleType::Linear {
                    "Max"
                } else {
                    "Max (data)"
                });
                let max_response =
                    ui.add_enabled(!auto, egui::DragValue::new(&mut max).speed(0.1));
                let changed = min_response.changed() || max_response.changed();
                queue_gesture_action(
                    &[&min_response, &max_response],
                    (changed && !auto).then_some(Action::SetAxisRange {
                        axis,
                        range: RangePolicy::Manual { min, max },
                    }),
                    actions,
                );
            });

            ui.separator();
            ui.label("Ticks");

            let mut major_auto = axis_ref.ticks.major_step.is_none();
            if ui.checkbox(&mut major_auto, "Auto major step").changed() {
                actions.push(Action::SetAxisMajorTickStep {
                    axis,
                    step: if major_auto { None } else { Some(1.0) },
                });
            }
            if !major_auto {
                let mut step = axis_ref.ticks.major_step.unwrap_or(1.0);
                let response = ui.add(
                    egui::DragValue::new(&mut step)
                        .speed(0.1)
                        .range(0.01..=1_000.0),
                );
                queue_continuous_action(
                    &response,
                    response.changed().then_some(Action::SetAxisMajorTickStep {
                        axis,
                        step: Some(step),
                    }),
                    actions,
                );
            }
            ui.small("Dense manual ticks are thinned to fit the current plot pixel size.");

            let mut number_format = axis_ref.ticks.number_format;
            ui.horizontal(|ui| {
                ui.label("Number format");
                egui::ComboBox::from_id_salt(("tick-number-format", format!("{axis:?}")))
                    .selected_text(tick_number_format_text(number_format))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut number_format, TickNumberFormat::Auto, "Auto");
                        for decimals in 0..=8 {
                            ui.selectable_value(
                                &mut number_format,
                                TickNumberFormat::Fixed { decimals },
                                format!("Fixed ({decimals})"),
                            );
                        }
                        for decimals in 0..=8 {
                            ui.selectable_value(
                                &mut number_format,
                                TickNumberFormat::Scientific { decimals },
                                format!("Scientific ({decimals})"),
                            );
                        }
                    });
            });
            if number_format != axis_ref.ticks.number_format {
                actions.push(Action::SetAxisTickNumberFormat {
                    axis,
                    format: number_format,
                });
            }

            let mut minor = axis_ref.ticks.minor_per_major;
            ui.horizontal(|ui| {
                ui.label("Minor per major");
                let response = ui.add(egui::DragValue::new(&mut minor).range(0..=20));
                queue_continuous_action(
                    &response,
                    response.changed().then_some(Action::SetAxisMinorTicks {
                        axis,
                        per_major: minor,
                    }),
                    actions,
                );
            });

            tick_mark_editor(
                ui,
                axis,
                false,
                axis_ref.ticks.major_ticks_visible,
                axis_ref.ticks.major_tick_length,
                actions,
            );
            tick_mark_editor(
                ui,
                axis,
                true,
                axis_ref.ticks.minor_ticks_visible,
                axis_ref.ticks.minor_tick_length,
                actions,
            );

            let mut major_grid = axis_ref.ticks.major_grid_visible;
            if ui.checkbox(&mut major_grid, "Major grid").changed() {
                actions.push(Action::SetAxisGridVisibility {
                    axis,
                    minor: false,
                    visible: major_grid,
                });
            }
            let mut minor_grid = axis_ref.ticks.minor_grid_visible;
            if ui.checkbox(&mut minor_grid, "Minor grid").changed() {
                actions.push(Action::SetAxisGridVisibility {
                    axis,
                    minor: true,
                    visible: minor_grid,
                });
            }
            grid_style_editor(
                ui,
                axis,
                false,
                axis_ref.ticks.major_grid_color,
                axis_ref.ticks.major_grid_width,
                actions,
            );
            grid_style_editor(
                ui,
                axis,
                true,
                axis_ref.ticks.minor_grid_color,
                axis_ref.ticks.minor_grid_width,
                actions,
            );
        });
    });
}

fn deferred_text_edit(
    ui: &mut egui::Ui,
    drafts: &mut HashMap<egui::Id, String>,
    id: egui::Id,
    hint: &str,
    model_value: &str,
) -> Option<String> {
    let mut value = drafts
        .get(&id)
        .cloned()
        .unwrap_or_else(|| model_value.to_owned());
    let response = ui
        .horizontal(|ui| {
            let label = ui.label(hint);
            let input_width = ui.available_width();
            ui.add(
                egui::TextEdit::singleline(&mut value)
                    .id(id)
                    .desired_width(input_width),
            )
            .labelled_by(label.id)
        })
        .inner;
    if response.has_focus() {
        drafts.insert(id, value.clone());
    }
    if response.lost_focus() {
        drafts.remove(&id);
        return (value != model_value).then_some(value);
    }
    None
}

fn queue_continuous_action(
    response: &egui::Response,
    action: Option<Action>,
    actions: &mut Vec<Action>,
) {
    queue_gesture_action(&[response], action, actions);
}

fn queue_color_gesture_begin(ui: &egui::Ui, response: &egui::Response, actions: &mut Vec<Action>) {
    let gesture_id = response.id.with("color-picker-continuous-edit");
    let active = ui.data(|data| data.get_temp::<bool>(gesture_id).unwrap_or(false));
    if (response.changed() || response.drag_started()) && !active {
        actions.push(Action::BeginContinuousEdit);
        ui.data_mut(|data| data.insert_temp(gesture_id, true));
    }
}

fn queue_color_gesture_end(ui: &egui::Ui, response: &egui::Response, actions: &mut Vec<Action>) {
    let gesture_id = response.id.with("color-picker-continuous-edit");
    let active = ui.data(|data| data.get_temp::<bool>(gesture_id).unwrap_or(false));
    let pointer_down = ui.ctx().input(|input| input.pointer.primary_down());
    if active && !pointer_down {
        actions.push(Action::EndContinuousEdit);
        ui.data_mut(|data| data.remove::<bool>(gesture_id));
    }
}

fn queue_gesture_action(
    responses: &[&egui::Response],
    action: Option<Action>,
    actions: &mut Vec<Action>,
) {
    if responses.iter().any(|response| response.drag_started()) {
        actions.push(Action::BeginContinuousEdit);
    }
    if let Some(action) = action {
        actions.push(action);
    }
    if responses.iter().any(|response| response.drag_stopped()) {
        actions.push(Action::EndContinuousEdit);
    }
}

fn range_contains_inclusive(range: &std::ops::Range<f64>, value: f64) -> bool {
    value.is_finite() && value >= range.start && value <= range.end
}

#[cfg(test)]
mod range_boundary_tests {
    use super::range_contains_inclusive;

    #[test]
    fn point_hit_testing_includes_all_four_axis_boundaries() {
        let x = 1.0..5.0;
        let y = -2.0..3.0;
        for point in [(1.0, -2.0), (1.0, 3.0), (5.0, -2.0), (5.0, 3.0)] {
            assert!(range_contains_inclusive(&x, point.0));
            assert!(range_contains_inclusive(&y, point.1));
        }
        assert!(!range_contains_inclusive(&x, 0.999));
        assert!(!range_contains_inclusive(&x, f64::NAN));
    }
}

fn error_column_combo(
    ui: &mut egui::Ui,
    series_id: crate::model::SeriesId,
    axis: AxisKind,
    label: &str,
    current: Option<String>,
    enabled: bool,
    columns: &[String],
    actions: &mut Vec<Action>,
) {
    let mut selected = current.clone();
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_label(label)
            .selected_text(selected.as_deref().unwrap_or("None"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, None, "None");
                for column in columns {
                    ui.selectable_value(&mut selected, Some(column.clone()), column);
                }
            });
    });
    if enabled && selected != current {
        actions.push(Action::SetSeriesErrorColumn {
            series_id,
            axis,
            column: selected,
        });
    }
}

fn asymmetric_error_column_combo(
    ui: &mut egui::Ui,
    series_id: crate::model::SeriesId,
    axis: AxisKind,
    side: ErrorBarSide,
    label: &str,
    current: Option<String>,
    columns: &[String],
    actions: &mut Vec<Action>,
) {
    let mut selected = current.clone();
    egui::ComboBox::from_label(label)
        .selected_text(selected.as_deref().unwrap_or("None"))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, None, "None");
            for column in columns {
                ui.selectable_value(&mut selected, Some(column.clone()), column);
            }
        });
    if selected != current {
        actions.push(Action::SetSeriesAsymmetricErrorColumn {
            series_id,
            axis,
            side,
            column: selected,
        });
    }
}

fn tick_mark_editor(
    ui: &mut egui::Ui,
    axis: AxisKind,
    minor: bool,
    visible: bool,
    length: f32,
    actions: &mut Vec<Action>,
) {
    let mut selected_visible = visible;
    let mut selected_length = length;
    let label = if minor { "Minor ticks" } else { "Major ticks" };
    let (visibility_response, length_response) = ui
        .horizontal(|ui| {
            let visibility_response = ui.checkbox(&mut selected_visible, label);
            let length_response = ui.add(
                egui::DragValue::new(&mut selected_length)
                    .range(0.0..=20.0)
                    .suffix(" px"),
            );
            (visibility_response, length_response)
        })
        .inner;
    let changed = visibility_response.changed() || length_response.changed();
    queue_continuous_action(
        &length_response,
        changed.then_some(Action::SetAxisTickMarkStyle {
            axis,
            minor,
            visible: selected_visible,
            length: selected_length,
        }),
        actions,
    );
}

fn grid_style_editor(
    ui: &mut egui::Ui,
    axis: AxisKind,
    minor: bool,
    color: crate::model::Color,
    width: f32,
    actions: &mut Vec<Action>,
) {
    let mut selected_color = color32_from_model(color);
    let color_response = color_edit_button(
        ui,
        if minor {
            "Minor grid color"
        } else {
            "Major grid color"
        },
        &mut selected_color,
    );
    let color_changed = color_response.changed();
    let mut selected_width = width;
    let width_response = ui.add(
        egui::Slider::new(&mut selected_width, 0.25..=4.0).text(if minor {
            "Minor width"
        } else {
            "Major width"
        }),
    );
    let width_changed = width_response.changed();
    queue_color_gesture_begin(ui, &color_response, actions);
    queue_gesture_action(
        &[&color_response, &width_response],
        (color_changed || width_changed).then_some(Action::SetAxisGridStyle {
            axis,
            minor,
            color: if color_changed {
                model_color_from_ui(selected_color)
            } else {
                color
            },
            width: selected_width,
        }),
        actions,
    );
    queue_color_gesture_end(ui, &color_response, actions);
}

fn line_style_text(value: LineStyle) -> &'static str {
    match value {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
    }
}

fn line_interpolation_text(value: LineInterpolation) -> &'static str {
    match value {
        LineInterpolation::Linear => "Linear",
        LineInterpolation::StepAfter => "Step after",
    }
}

fn marker_shape_text(value: MarkerShape) -> &'static str {
    match value {
        MarkerShape::Circle => "Circle",
        MarkerShape::Square => "Square",
        MarkerShape::Triangle => "Triangle",
        MarkerShape::Cross => "Cross",
    }
}

fn color32_from_model(color: crate::model::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn color_edit_button(ui: &mut egui::Ui, label: &str, color: &mut Color32) -> egui::Response {
    ui.horizontal(|ui| {
        let label_response = ui.label(label);
        ui.color_edit_button_srgba(color)
            .labelled_by(label_response.id)
    })
    .inner
}

fn model_color_from_ui(color: Color32) -> crate::model::Color {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    crate::model::Color { r, g, b, a }
}

fn tick_number_format_text(value: TickNumberFormat) -> String {
    match value {
        TickNumberFormat::Auto => "Auto".to_owned(),
        TickNumberFormat::Fixed { decimals } => format!("Fixed ({decimals})"),
        TickNumberFormat::Scientific { decimals } => format!("Scientific ({decimals})"),
    }
}

fn distance_to_segment(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * projection)
}

fn format_measurement(value: f64) -> String {
    value.to_string()
}

fn clip_segment_to_rect(
    start: egui::Pos2,
    end: egui::Pos2,
    rect: egui::Rect,
) -> Option<(egui::Pos2, egui::Pos2)> {
    let direction = end - start;
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for (p, q) in [
        (-direction.x, start.x - rect.left()),
        (direction.x, rect.right() - start.x),
        (-direction.y, start.y - rect.top()),
        (direction.y, rect.bottom() - start.y),
    ] {
        if p.abs() <= f32::EPSILON {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let ratio = q / p;
        if p < 0.0 {
            lower = lower.max(ratio);
        } else {
            upper = upper.min(ratio);
        }
        if lower > upper {
            return None;
        }
    }
    Some((start + direction * lower, start + direction * upper))
}

fn fit_aspect_rect(bounds: egui::Rect, aspect_ratio: f32) -> egui::Rect {
    if !aspect_ratio.is_finite()
        || aspect_ratio <= 0.0
        || bounds.width() <= 0.0
        || bounds.height() <= 0.0
    {
        return bounds;
    }
    let available_ratio = bounds.width() / bounds.height();
    let size = if available_ratio > aspect_ratio {
        egui::vec2(bounds.height() * aspect_ratio, bounds.height())
    } else {
        egui::vec2(bounds.width(), bounds.width() / aspect_ratio)
    };
    egui::Rect::from_center_size(bounds.center(), size)
}

fn scale_plot_model_dimensions(model: &mut PlotModel, factor: f32) {
    let scale_u32 = |value: u32| (value as f32 * factor).round().clamp(1.0, u32::MAX as f32) as u32;
    let scale_area =
        |value: u32| (value as f32 * factor).round().clamp(0.0, u32::MAX as f32) as u32;
    let scale_f32 = |value: f32| value * factor;
    model.layout.margin = scale_area(model.layout.margin);
    model.layout.title_font_size = scale_u32(model.layout.title_font_size);
    model.layout.x_label_area_size = scale_area(model.layout.x_label_area_size);
    model.layout.y_label_area_size = scale_area(model.layout.y_label_area_size);
    for axis in [&mut model.axes.x, &mut model.axes.y] {
        axis.label_font_size = scale_u32(axis.label_font_size);
        axis.axis_title_font_size = scale_u32(axis.axis_title_font_size);
        axis.ticks.major_tick_length = scale_f32(axis.ticks.major_tick_length);
        axis.ticks.minor_tick_length = scale_f32(axis.ticks.minor_tick_length);
        axis.ticks.major_grid_width = scale_f32(axis.ticks.major_grid_width);
        axis.ticks.minor_grid_width = scale_f32(axis.ticks.minor_grid_width);
    }
    model.legend.font_size = scale_u32(model.legend.font_size);
    model.legend.padding = scale_area(model.legend.padding);
    model.legend.row_spacing = scale_area(model.legend.row_spacing);
    model.legend.sample_length = scale_u32(model.legend.sample_length);
    model.legend.column_spacing = scale_area(model.legend.column_spacing);
    model.legend.border_width = scale_f32(model.legend.border_width);
    for series in &mut model.series {
        scale_series_dimensions(series, factor);
    }
    for annotation in &mut model.annotations {
        annotation.font_size = scale_u32(annotation.font_size);
    }
}

fn preview_scale_for_width(
    screen_width_points: f32,
    pixels_per_point: f32,
    output_width: u32,
) -> f32 {
    screen_width_points * pixels_per_point / output_width.max(1) as f32
}

fn scale_series_dimensions(series: &mut crate::model::SeriesModel, factor: f32) {
    series.style.line_width *= factor;
    series.style.marker_preset.size *= factor;
    if let Some(marker) = &mut series.style.marker {
        marker.size *= factor;
    }
}

fn font_family_text(value: &PlotFontFamily) -> String {
    match value {
        PlotFontFamily::SansSerif => "Sans Serif".to_owned(),
        PlotFontFamily::Serif => "Serif".to_owned(),
        PlotFontFamily::Monospace => "Monospace".to_owned(),
        PlotFontFamily::Custom(name) => format!("Custom: {name}"),
        PlotFontFamily::CustomAsset { family, asset_id } => {
            format!("Custom: {family} ({})", &asset_id[..8.min(asset_id.len())])
        }
    }
}

fn import_delimiter_text(value: ImportDelimiter) -> &'static str {
    match value {
        ImportDelimiter::Comma => "Comma",
        ImportDelimiter::Semicolon => "Semicolon",
        ImportDelimiter::Tab => "Tab",
        ImportDelimiter::Whitespace => "Whitespace",
    }
}

fn scale_text(value: ScaleType) -> &'static str {
    match value {
        ScaleType::Linear => "Linear",
        ScaleType::Log10 => "Plot log10(data)",
        ScaleType::Log10Axis => "Log10 axis (1, 10, 100)",
        ScaleType::LogE => "Natural log of data",
    }
}

fn panel_axis_controls(
    ui: &mut egui::Ui,
    panel_id: u64,
    view: &mut crate::model::PanelAxisView,
    controller: &crate::controller::PlotController,
    actions: &mut Vec<Action>,
) -> Option<egui::Rect> {
    let mut y_min_rect = None;
    let before = view.clone();
    for (axis, scale) in [
        (AxisKind::X, &mut view.x_scale),
        (AxisKind::Y, &mut view.y_scale),
    ] {
        egui::ComboBox::from_label(format!("Panel {} scale", axis_name(axis)))
            .selected_text(scale_text(*scale))
            .show_ui(ui, |ui| {
                for (value, label) in [
                    (ScaleType::Linear, "Linear"),
                    (ScaleType::Log10, "Log10(data)"),
                    (ScaleType::Log10Axis, "Log10 axis (1, 10, 100)"),
                    (ScaleType::LogE, "Ln(data)"),
                ] {
                    ui.selectable_value(scale, value, label);
                }
            });
        let range = if axis == AxisKind::X {
            &mut view.x_range
        } else {
            &mut view.y_range
        };
        if *scale != ScaleType::Linear
            && matches!(range, RangePolicy::Manual { min, max } if *min <= 0.0 || *max <= 0.0)
        {
            *range = RangePolicy::Auto;
        }
    }
    if *view != before {
        actions.push(Action::SetPanelAxisView {
            panel_id,
            view: Some(view.clone()),
        });
    }
    for axis in [AxisKind::X, AxisKind::Y] {
        let mut range = if axis == AxisKind::X {
            view.x_range.clone()
        } else {
            view.y_range.clone()
        };
        let mut automatic = matches!(range, RangePolicy::Auto);
        let auto_changed = ui
            .checkbox(&mut automatic, format!("Auto {} range", axis_name(axis)))
            .changed();
        if auto_changed {
            if automatic {
                range = RangePolicy::Auto;
            } else {
                let suggested = controller
                    .suggested_axis_range_for_panel(panel_id, axis)
                    .or_else(|| controller.suggested_axis_range(axis))
                    .unwrap_or((-1.0, 1.0));
                range = RangePolicy::Manual {
                    min: suggested.0,
                    max: suggested.1,
                };
            }
        }
        let mut drag_responses = None;
        let mut drag_action = None;
        let mut changed_valid_range = false;
        if let RangePolicy::Manual { min, max } = &range {
            let mut next_min = *min;
            let mut next_max = *max;
            let (min_response, max_response) = ui
                .horizontal(|ui| {
                    ui.label(format!("{} min", axis_name(axis)));
                    let min_response = ui.add(egui::DragValue::new(&mut next_min).speed(0.1));
                    ui.label("max");
                    let max_response = ui.add(egui::DragValue::new(&mut next_max).speed(0.1));
                    (min_response, max_response)
                })
                .inner;
            let changed = min_response.changed() || max_response.changed();
            if axis == AxisKind::Y {
                y_min_rect = Some(min_response.rect);
            }
            drag_responses = Some((min_response, max_response));
            let valid = next_min.is_finite()
                && next_max.is_finite()
                && next_min < next_max
                && (if axis == AxisKind::X {
                    view.x_scale
                } else {
                    view.y_scale
                } == ScaleType::Linear
                    || (next_min > 0.0 && next_max > 0.0));
            if changed && valid {
                range = RangePolicy::Manual {
                    min: next_min,
                    max: next_max,
                };
                changed_valid_range = true;
            } else if changed {
                ui.colored_label(Color32::RED, "Enter min < max (positive on log axes)");
            }
        }
        if axis == AxisKind::X {
            view.x_range = range;
        } else {
            view.y_range = range;
        }
        if changed_valid_range {
            drag_action = Some(Action::SetPanelAxisView {
                panel_id,
                view: Some(view.clone()),
            });
        }
        if auto_changed {
            actions.push(Action::SetPanelAxisView {
                panel_id,
                view: Some(view.clone()),
            });
        }
        if let Some((min_response, max_response)) = drag_responses {
            queue_gesture_action(&[&min_response, &max_response], drag_action, actions);
        }
    }
    y_min_rect
}

fn axis_name(axis: AxisKind) -> &'static str {
    match axis {
        AxisKind::X => "X",
        AxisKind::Y => "Y",
    }
}

fn build_preview_panel(
    controller: &PlotController,
    panel: &PlotPanel,
    preview_scale: f32,
    linked_x_range: std::ops::Range<f64>,
    linked_y_range: std::ops::Range<f64>,
) -> Result<PreviewPanelData, String> {
    let axes = controller
        .effective_panel_axis_view(panel.id)
        .unwrap_or_else(|| PanelAxisView {
            x_scale: controller.model.axes.x.scale,
            y_scale: controller.model.axes.y.scale,
            x_range: controller.model.axes.x.range.clone(),
            y_range: controller.model.axes.y.range.clone(),
        });
    let mut series_data = Vec::new();
    let mut data_points = Vec::new();
    for series in controller
        .model
        .series
        .iter()
        .filter(|series| series.visible && panel.series_ids.contains(&series.id))
    {
        let points = controller
            .points_for_series(series.id)
            .map_err(|error| error.to_string())?;
        let (scaled, line_segments) =
            crate::controller::scale_points_into_segments(&points, axes.x_scale, axes.y_scale);
        let error_bars = controller
            .error_bars_for_series_with_scales(series.id, axes.x_scale, axes.y_scale)
            .map_err(|error| error.to_string())?;
        let confidence_band_segments = controller
            .confidence_band_segments_for_series_with_scales(series.id, axes.x_scale, axes.y_scale)
            .map_err(|error| error.to_string())?;
        let area_baseline = series
            .style
            .area_fill_visible
            .then_some(series.style.area_baseline)
            .flatten()
            .and_then(|baseline| crate::controller::scale_value(baseline, axes.y_scale));
        data_points.extend(scaled.iter().copied());
        data_points.extend(
            error_bars
                .bars
                .iter()
                .flat_map(crate::view::series::ErrorBar::range_points),
        );
        if let Some(baseline) = area_baseline {
            data_points.extend(scaled.iter().map(|(x, _)| (*x, baseline)));
        }
        data_points.extend(confidence_band_segments.iter().flat_map(|segment| {
            segment
                .iter()
                .flat_map(|(x, low, high)| [(*x, *low), (*x, *high)])
        }));
        let mut render_series = series.clone();
        scale_series_dimensions(&mut render_series, preview_scale);
        series_data.push(PreviewSeriesData {
            series: render_series,
            points: scaled,
            line_segments,
            error_bars,
            area_baseline,
            confidence_band_segments,
        });
    }
    let x_range = if panel.axis_view.is_none() {
        linked_x_range
    } else {
        resolve_data_range(
            &axes.x_range,
            data_points.iter().copied(),
            true,
            -1.0..1.0,
            axes.x_scale,
        )
    };
    let y_range = if panel.axis_view.is_none() {
        linked_y_range
    } else {
        resolve_data_range(&axes.y_range, data_points, false, -1.0..1.0, axes.y_scale)
    };
    Ok(PreviewPanelData {
        panel: panel.clone(),
        axes,
        x_range,
        y_range,
        series: series_data,
    })
}

fn panel_legend_series(states: &[PreviewPanelData]) -> Vec<&PreviewSeriesData> {
    let mut seen = std::collections::HashSet::new();
    states
        .iter()
        .flat_map(|state| state.series.iter())
        .filter(|data| !data.points.is_empty() && seen.insert(data.series.id))
        .collect()
}

fn legend_position_text(value: LegendPosition) -> &'static str {
    match value {
        LegendPosition::Custom => "Custom (drag)",
        LegendPosition::TopLeft => "Top Left",
        LegendPosition::TopRight => "Top Right",
        LegendPosition::BottomLeft => "Bottom Left",
        LegendPosition::BottomRight => "Bottom Right",
        LegendPosition::OutsideLeft => "Outside Left",
        LegendPosition::OutsideRight => "Outside Right",
        LegendPosition::OutsideTop => "Outside Top",
        LegendPosition::OutsideBottom => "Outside Bottom",
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn preview_physical_scale_responds_to_output_pixels_and_display_density() {
        let ordinary = preview_scale_for_width(480.0, 1.0, 1_000);
        let high_dpi_export = preview_scale_for_width(480.0, 1.0, 2_000);
        let high_density_display = preview_scale_for_width(480.0, 2.0, 1_000);
        assert!((ordinary - 0.48).abs() < 1e-6);
        assert!((high_dpi_export - ordinary / 2.0).abs() < 1e-6);
        assert!((high_density_display - ordinary * 2.0).abs() < 1e-6);
    }

    #[test]
    fn preview_scaling_changes_display_sizes_without_changing_data_or_bindings() {
        let mut model = PlotModel::default();
        model.layout.title_font_size = 20;
        model.layout.margin = 10;
        model.legend.font_size = 16;
        model.series[0].x_column = "time".to_owned();
        model.series[0].y_column = "pressure".to_owned();
        model.series[0].style.line_width = 2.0;
        model.series[0].style.marker = Some(crate::model::MarkerStyle {
            shape: crate::model::MarkerShape::Square,
            size: 8.0,
        });
        let original_range = model.axes.x.range.clone();
        scale_plot_model_dimensions(&mut model, 0.5);
        assert_eq!(model.layout.title_font_size, 10);
        assert_eq!(model.layout.margin, 5);
        assert_eq!(model.legend.font_size, 8);
        assert_eq!(model.series[0].style.line_width, 1.0);
        assert_eq!(model.series[0].style.marker.as_ref().unwrap().size, 4.0);
        assert_eq!(model.series[0].x_column, "time");
        assert_eq!(model.series[0].y_column, "pressure");
        assert_eq!(model.axes.x.range, original_range);
    }

    #[test]
    fn measurement_text_round_trips_small_and_close_values() {
        for value in [
            1e-14,
            1_000_000.0,
            1_000_000.000_000_1,
            -std::f64::consts::PI,
        ] {
            let formatted = format_measurement(value);
            let parsed = formatted.parse::<f64>().unwrap();
            assert_eq!(parsed, value, "formatted measurement: {formatted}");
        }
        assert_ne!(
            format_measurement(1_000_000.0),
            format_measurement(1_000_000.000_000_1)
        );
    }

    #[test]
    fn export_preview_rect_matches_target_aspect_ratio_and_stays_inside_available_area() {
        let available = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(900.0, 700.0));
        let preview = fit_aspect_rect(available, 16.0 / 9.0);
        assert!((preview.width() / preview.height() - 16.0 / 9.0).abs() < 0.001);
        assert!(available.contains(preview.min));
        assert!(available.contains(preview.max));
    }

    #[test]
    fn zoom_keeps_the_value_under_the_cursor_fixed() {
        let original = 0.0..100.0;
        let anchor_fraction = 0.25;
        let anchor = original.start + (original.end - original.start) * anchor_fraction;
        let zoomed = zoom_range(&original, anchor_fraction, 0.5).unwrap();
        let zoomed_anchor = zoomed.start + (zoomed.end - zoomed.start) * anchor_fraction;
        assert!((anchor - zoomed_anchor).abs() < f64::EPSILON * 10.0);
        assert_eq!(zoomed.end - zoomed.start, 50.0);
    }

    #[test]
    fn panning_and_zooming_log_ranges_return_positive_raw_bounds() {
        let transformed = 0.0..3.0;
        let panned = pan_range(&transformed, 10.0, 100.0).unwrap();
        let zoomed = zoom_range(&panned, 0.5, 0.75).unwrap();
        let raw = to_data_range(zoomed, ScaleType::Log10).unwrap();
        assert!(matches!(raw, RangePolicy::Manual { min, max } if min > 0.0 && max > min));
    }
}
