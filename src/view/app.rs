//! Main editor window composition.
//!
//! Dataflow: render from controller state, collect user interactions, emit actions.

use std::ops::Range;

use crate::controller::action::Action;
use crate::controller::{NotificationLevel, PlotController};
use crate::model::{
    AxisKind, LegendPosition, LineStyle, MarkerShape, RangePolicy, ScaleType, TickConfig,
};
use crate::view::FilesMenu;
use eframe::egui::{self, Color32, RichText, SidePanel};
use egui_plotter::{Chart, MouseConfig};
use plotters::coord::types::RangedCoordf32;
use plotters::prelude::*;
use plotters::style::Color as PlottersColor;

/// EN: Main editor view. Contains menu, control panels and plot area.
/// RU: Osnovnoe predstavlenie. Soderzhit menyu, paneli upravleniya i oblast grafa.
pub struct PlotEditorView;

impl PlotEditorView {
    /// Creates editor view state.
    pub fn new() -> Self {
        Self
    }

    /// Renders full UI and collects actions requested by user interactions.
    pub fn draw(&mut self, ctx: &egui::Context, controller: &PlotController) -> Vec<Action> {
        let mut actions = FilesMenu::draw(ctx);
        self.draw_controls(ctx, controller, &mut actions);
        self.draw_plot(ctx, controller);
        actions
    }

    fn draw_controls(
        &mut self,
        ctx: &egui::Context,
        controller: &PlotController,
        actions: &mut Vec<Action>,
    ) {
        SidePanel::right("control_panel")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Controls");
                ui.separator();

                if let Some(n) = controller.notification() {
                    let color = match n.level {
                        NotificationLevel::Info => Color32::DARK_GREEN,
                        NotificationLevel::Error => Color32::RED,
                    };
                    ui.colored_label(color, &n.message);
                    ui.separator();
                }

                ui.label(RichText::new("Axis").strong());
                axis_editor(ui, "X Axis", AxisKind::X, controller, actions);
                axis_editor(ui, "Y Axis", AxisKind::Y, controller, actions);

                ui.separator();
                ui.label(RichText::new("Legend").strong());
                let mut legend_visible = controller.model.legend.visible;
                if ui.checkbox(&mut legend_visible, "Visible").changed() {
                    actions.push(Action::SetLegendVisible(legend_visible));
                }
                let mut legend_title = controller.model.legend.title.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut legend_title).changed() {
                    actions.push(Action::SetLegendTitle(if legend_title.trim().is_empty() {
                        None
                    } else {
                        Some(legend_title)
                    }));
                }
                let mut legend_pos = controller.model.legend.position;
                egui::ComboBox::from_label("Position")
                    .selected_text(legend_position_text(legend_pos))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut legend_pos, LegendPosition::TopLeft, "Top Left");
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
                    });
                if legend_pos != controller.model.legend.position {
                    actions.push(Action::SetLegendPosition(legend_pos));
                }
                let mut legend_font_size = controller.model.legend.font_size;
                if ui
                    .add(
                        egui::Slider::new(&mut legend_font_size, 8..=64)
                            .text("Legend font size"),
                    )
                    .changed()
                {
                    actions.push(Action::SetLegendFontSize(legend_font_size));
                }
                let mut legend_color = Color32::from_rgba_premultiplied(
                    controller.model.legend.font_color.r,
                    controller.model.legend.font_color.g,
                    controller.model.legend.font_color.b,
                    controller.model.legend.font_color.a,
                );
                if ui.color_edit_button_srgba(&mut legend_color).changed() {
                    actions.push(Action::SetLegendFontColor(crate::model::Color {
                        r: legend_color.r(),
                        g: legend_color.g(),
                        b: legend_color.b(),
                        a: legend_color.a(),
                    }));
                }

                ui.separator();
                ui.label(RichText::new("Label").strong());
                let mut title = controller.model.layout.title.clone();
                if ui.text_edit_singleline(&mut title).changed() {
                    actions.push(Action::SetChartTitle(title));
                }
                let mut title_font_size = controller.model.layout.title_font_size;
                if ui
                    .add(egui::Slider::new(&mut title_font_size, 8..=72).text("Title font size"))
                    .changed()
                {
                    actions.push(Action::SetLabelFontSize(title_font_size));
                }
                let mut title_color = Color32::from_rgba_premultiplied(
                    controller.model.layout.title_font_color.r,
                    controller.model.layout.title_font_color.g,
                    controller.model.layout.title_font_color.b,
                    controller.model.layout.title_font_color.a,
                );
                if ui.color_edit_button_srgba(&mut title_color).changed() {
                    actions.push(Action::SetLabelFontColor(crate::model::Color {
                        r: title_color.r(),
                        g: title_color.g(),
                        b: title_color.b(),
                        a: title_color.a(),
                    }));
                }

                ui.separator();
                ui.label(RichText::new("Series").strong());
                let columns = controller.available_columns();

                for series in &controller.model.series {
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

                        let mut name = series.name.clone();
                        if ui.text_edit_singleline(&mut name).changed() {
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
                            let mut x_col = if series.x_column.is_empty() {
                                columns[0].clone()
                            } else {
                                series.x_column.clone()
                            };
                            egui::ComboBox::from_label("X column")
                                .selected_text(x_col.clone())
                                .show_ui(ui, |ui| {
                                    for col in &columns {
                                        ui.selectable_value(&mut x_col, col.clone(), col);
                                    }
                                });
                            if x_col != series.x_column {
                                actions.push(Action::SetSeriesXColumn {
                                    series_id: series.id,
                                    x_column: x_col,
                                });
                            }

                            let default_y =
                                columns.get(1).cloned().unwrap_or_else(|| columns[0].clone());
                            let mut y_col = if series.y_column.is_empty() {
                                default_y
                            } else {
                                series.y_column.clone()
                            };
                            egui::ComboBox::from_label("Y column")
                                .selected_text(y_col.clone())
                                .show_ui(ui, |ui| {
                                    for col in &columns {
                                        ui.selectable_value(&mut y_col, col.clone(), col);
                                    }
                                });
                            if y_col != series.y_column {
                                actions.push(Action::SetSeriesYColumn {
                                    series_id: series.id,
                                    y_column: y_col,
                                });
                            }
                        }

                        let mut width = series.style.line_width;
                        if ui
                            .add(egui::Slider::new(&mut width, 1.0..=10.0).text("Width"))
                            .changed()
                        {
                            actions.push(Action::SetSeriesLineWidth {
                                series_id: series.id,
                                width,
                            });
                        }

                        let mut style = series.style.line_style;
                        egui::ComboBox::from_label("Line style")
                            .selected_text(line_style_text(style))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut style, LineStyle::Solid, "Solid");
                                ui.selectable_value(&mut style, LineStyle::Dashed, "Dashed");
                                ui.selectable_value(&mut style, LineStyle::Dotted, "Dotted");
                            });
                        if style != series.style.line_style {
                            actions.push(Action::SetSeriesLineStyle {
                                series_id: series.id,
                                line_style: style,
                            });
                        }

                        let mut color = Color32::from_rgba_premultiplied(
                            series.style.color.r,
                            series.style.color.g,
                            series.style.color.b,
                            series.style.color.a,
                        );
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            actions.push(Action::SetSeriesColor {
                                series_id: series.id,
                                color: crate::model::Color {
                                    r: color.r(),
                                    g: color.g(),
                                    b: color.b(),
                                    a: color.a(),
                                },
                            });
                        }

                        let mut marker_enabled = series.style.marker.is_some();
                        if ui.checkbox(&mut marker_enabled, "Marker").changed() {
                            actions.push(Action::SetSeriesMarker {
                                series_id: series.id,
                                marker: if marker_enabled {
                                    Some(MarkerShape::Circle)
                                } else {
                                    None
                                },
                                size: series.style.marker.as_ref().map(|m| m.size).unwrap_or(3.0),
                            });
                        }
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
                ui.horizontal(|ui| {
                    if ui.button("Undo").clicked() {
                        actions.push(Action::Undo);
                    }
                    if ui.button("Redo").clicked() {
                        actions.push(Action::Redo);
                    }
                });
            });
    }

    fn draw_plot(&mut self, ctx: &egui::Context, controller: &PlotController) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !controller.has_data() {
                ui.heading("No data loaded");
                ui.label("Use Files > From CSV or Files > From TXT");
                return;
            }

            let mut rendered = Vec::new();
            for series in &controller.model.series {
                if !series.visible {
                    continue;
                }
                if let Ok(points) = controller.points_for_series(series.id) {
                    let scaled = points
                        .iter()
                        .copied()
                        .filter_map(|(x, y)| {
                            apply_scale(
                                x,
                                y,
                                controller.model.axes.x.scale,
                                controller.model.axes.y.scale,
                            )
                        })
                        .collect::<Vec<_>>();
                    rendered.push((series.clone(), scaled));
                }
            }

            let x_range = resolve_range(
                &controller.model.axes.x.range,
                &rendered,
                true,
                -1.0..1.0,
            );
            let y_range = resolve_range(
                &controller.model.axes.y.range,
                &rendered,
                false,
                -1.0..1.0,
            );

            let title = controller.model.layout.title.clone();
            let x_label = format!(
                "{}{}",
                controller.model.axes.x.label,
                scale_suffix(controller.model.axes.x.scale)
            );
            let y_label = format!(
                "{}{}",
                controller.model.axes.y.label,
                scale_suffix(controller.model.axes.y.scale)
            );
            let x_ticks = controller.model.axes.x.ticks.clone();
            let y_ticks = controller.model.axes.y.ticks.clone();
            let x_label_font_size = controller.model.axes.x.label_font_size;
            let y_label_font_size = controller.model.axes.y.label_font_size;
            let legend_visible = controller.model.legend.visible;
            let legend_pos = controller.model.legend.position;
            let legend_font_size = controller.model.legend.font_size;
            let legend_font_color = RGBColor(
                controller.model.legend.font_color.r,
                controller.model.legend.font_color.g,
                controller.model.legend.font_color.b,
            );
            let margin = controller.model.layout.margin;
            let x_label_area = controller.model.layout.x_label_area_size;
            let y_label_area = controller.model.layout.y_label_area_size;
            let title_font_size = controller.model.layout.title_font_size;
            let title_font_color = RGBColor(
                controller.model.layout.title_font_color.r,
                controller.model.layout.title_font_color.g,
                controller.model.layout.title_font_color.b,
            );

            let mut chart = Chart::new((x_range.clone(), y_range.clone()))
                .mouse(MouseConfig::enabled())
                .builder_cb(Box::new(move |area, _t, _ranges| {
                    let mut chart = ChartBuilder::on(area)
                        .caption(
                            title.clone(),
                            ("sans-serif", title_font_size)
                                .into_font()
                                .color(&title_font_color),
                        )
                        .margin(margin)
                        .x_label_area_size(x_label_area)
                        .y_label_area_size(y_label_area)
                        .build_cartesian_2d(x_range.clone(), y_range.clone())
                        .expect("build chart failed");

                    configure_mesh(
                        &mut chart,
                        &x_label,
                        &y_label,
                        x_label_font_size,
                        y_label_font_size,
                        &x_ticks,
                        &y_ticks,
                        x_range.clone(),
                        y_range.clone(),
                    );

                    for (series, points) in &rendered {
                        if points.is_empty() {
                            continue;
                        }
                        let color = RGBColor(
                            series.style.color.r,
                            series.style.color.g,
                            series.style.color.b,
                        );
                        let style = ShapeStyle::from(&color)
                            .stroke_width(series.style.line_width.max(1.0) as u32);

                        if series.style.line_style == LineStyle::Dotted {
                            let _ = chart.draw_series(
                                points
                                    .iter()
                                    .map(|(x, y)| Circle::new((*x, *y), 2, style.filled())),
                            );
                        } else {
                            let drawn = chart
                                .draw_series(LineSeries::new(points.iter().copied(), style))
                                .expect("draw series failed");
                            if legend_visible {
                                drawn.label(series.name.clone()).legend(move |(x, y)| {
                                    PathElement::new(vec![(x, y), (x + 20, y)], color)
                                });
                            }
                        }
                    }

                    if legend_visible {
                        chart
                            .configure_series_labels()
                            .label_font(
                                ("sans-serif", legend_font_size)
                                    .into_font()
                                    .color(&legend_font_color),
                            )
                            .position(series_label_position(legend_pos))
                            .background_style(WHITE.mix(0.8))
                            .border_style(BLACK)
                            .draw()
                            .expect("draw legend failed");
                    }
                }));

            chart.draw(ui);
        });
    }
}

fn axis_editor(
    ui: &mut egui::Ui,
    title: &str,
    axis: AxisKind,
    controller: &PlotController,
    actions: &mut Vec<Action>,
) {
    ui.push_id(title, |ui| {
        ui.collapsing(title, |ui| {
        let axis_ref = match axis {
            AxisKind::X => &controller.model.axes.x,
            AxisKind::Y => &controller.model.axes.y,
        };

        let mut label = axis_ref.label.clone();
        if ui.text_edit_singleline(&mut label).changed() {
            actions.push(Action::SetAxisLabel { axis, label });
        }
        ui.horizontal(|ui| {
            let mut font_size = axis_ref.label_font_size;
            let mut changed = false;
            changed |= ui
                .add(egui::Slider::new(&mut font_size, 8..=72).text("Label font"))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut font_size).range(8..=200))
                .changed();
            if changed && font_size != axis_ref.label_font_size {
                actions.push(Action::SetAxisLabelFontSize { axis, font_size });
            }
        });

        let mut scale = axis_ref.scale;
        egui::ComboBox::from_label("Scale")
            .selected_text(scale_text(scale))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut scale, ScaleType::Linear, "Linear");
                ui.selectable_value(&mut scale, ScaleType::Log10, "Log10");
                ui.selectable_value(&mut scale, ScaleType::LogE, "LogE");
            });
        if scale != axis_ref.scale {
            actions.push(Action::SetAxisScale { axis, scale });
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
                actions.push(Action::SetAxisRange {
                    axis,
                    range: RangePolicy::Manual {
                        min: -1.0,
                        max: 1.0,
                    },
                });
            }
        });

        let (mut min, mut max) = match axis_ref.range {
            RangePolicy::Auto => (-1.0, 1.0),
            RangePolicy::Manual { min, max } => (min, max),
        };
        ui.horizontal(|ui| {
            ui.label("Min");
            let min_changed = ui.add(egui::DragValue::new(&mut min).speed(0.1)).changed();
            ui.label("Max");
            let max_changed = ui.add(egui::DragValue::new(&mut max).speed(0.1)).changed();
            if (min_changed || max_changed) && !auto {
                actions.push(Action::SetAxisRange {
                    axis,
                    range: RangePolicy::Manual { min, max },
                });
            }
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
            if ui
                .add(
                    egui::DragValue::new(&mut step)
                        .speed(0.1)
                        .range(0.01..=1_000.0),
                )
                .changed()
            {
                actions.push(Action::SetAxisMajorTickStep {
                    axis,
                    step: Some(step),
                });
            }
        }

        let mut minor = axis_ref.ticks.minor_per_major;
        ui.horizontal(|ui| {
            ui.label("Minor per major");
            if ui
                .add(egui::DragValue::new(&mut minor).range(0..=20))
                .changed()
            {
                actions.push(Action::SetAxisMinorTicks {
                    axis,
                    per_major: minor,
                });
            }
        });
        });
    });
}

fn resolve_range(
    policy: &RangePolicy,
    data: &[(crate::model::SeriesModel, Vec<(f32, f32)>)],
    is_x: bool,
    fallback: Range<f32>,
) -> Range<f32> {
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
    x_label: &str,
    y_label: &str,
    x_label_font_size: u32,
    y_label_font_size: u32,
    x_ticks: &TickConfig,
    y_ticks: &TickConfig,
    x_range: Range<f32>,
    y_range: Range<f32>,
) {
    let x_labels = labels_from_step(x_range, x_ticks.major_step).unwrap_or(10);
    let y_labels = labels_from_step(y_range, y_ticks.major_step).unwrap_or(10);

    chart
        .configure_mesh()
        .x_desc(x_label)
        .y_desc(y_label)
        .axis_desc_style(("sans-serif", x_label_font_size.max(y_label_font_size)))
        .x_labels(x_labels)
        .y_labels(y_labels)
        .max_light_lines(x_ticks.minor_per_major.max(y_ticks.minor_per_major) as usize)
        .draw()
        .expect("draw mesh failed");
}

fn labels_from_step(range: Range<f32>, step: Option<f64>) -> Option<usize> {
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

fn line_style_text(value: LineStyle) -> &'static str {
    match value {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
    }
}

fn scale_text(value: ScaleType) -> &'static str {
    match value {
        ScaleType::Linear => "Linear",
        ScaleType::Log10 => "Log10",
        ScaleType::LogE => "LogE",
    }
}

fn scale_suffix(value: ScaleType) -> &'static str {
    match value {
        ScaleType::Linear => "",
        ScaleType::Log10 => " [log10]",
        ScaleType::LogE => " [ln]",
    }
}

fn legend_position_text(value: LegendPosition) -> &'static str {
    match value {
        LegendPosition::TopLeft => "Top Left",
        LegendPosition::TopRight => "Top Right",
        LegendPosition::BottomLeft => "Bottom Left",
        LegendPosition::BottomRight => "Bottom Right",
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


