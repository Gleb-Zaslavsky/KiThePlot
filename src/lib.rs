//! Tiny, embeddable plot redactor crate.
//!
//! Architecture summary:
//! - `model`: domain data structures (`DataTable`, plot configuration, styles).
//! - `controller`: action handling, validation, undo/redo, import/export orchestration.
//! - `view`: egui UI composition and action emission.
//!
//! Dataflow:
//! `View -> Action -> Controller -> Model`, then View re-renders from model snapshot.

mod atomic_file;
pub mod controller;
mod export;
pub mod font;
pub mod model;
pub mod view;

pub use font::register_chart_font;

use controller::PlotController;
use eframe::egui;
use view::PlotEditorView;

/// EN: Embeddable editor app object for host applications.
/// RU: Vstraivaemy obekt redaktora dlya host-prilozheniy.
pub struct PlotEditorApp {
    controller: PlotController,
    view: PlotEditorView,
}

impl PlotEditorApp {
    /// Creates an editor app with empty data and default plot settings.
    pub fn new() -> Self {
        Self::with_model(model::PlotModel::default())
    }

    /// Creates an embeddable editor with host-provided initial plot settings and series styles.
    /// The host can load data afterward through `controller_mut().load_from_data_source(...)`.
    pub fn with_model(model: model::PlotModel) -> Self {
        Self {
            controller: PlotController::with_model(model),
            view: PlotEditorView::new(),
        }
    }

    /// Gives mutable access to controller for host-application integration.
    pub fn controller_mut(&mut self) -> &mut PlotController {
        &mut self.controller
    }
}

impl Default for PlotEditorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for PlotEditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let actions = self.view.draw(ui, &self.controller);
        for action in actions {
            let _ = self.controller.dispatch(action);
        }
    }
}

/// EN: Convenience runner for standalone usage.
/// RU: Udobny zapusk dlya standalone-ispolzovaniya.
pub fn run_native() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "KiThe Plot Redactor",
        native_options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            Ok(Box::new(PlotEditorApp::new()))
        }),
    )
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{NodeT as _, Queryable as _},
    };

    struct EditorFixture {
        controller: PlotController,
        view: PlotEditorView,
    }

    struct SnapshotData;

    impl model::DataSource for SnapshotData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "time" => Some((0..25).map(|index| index as f64 * 0.25).collect()),
                "signal" => Some((0..25).map(|index| (index as f64 * 0.25).sin()).collect()),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            vec!["time".to_owned(), "signal".to_owned()]
        }

        fn len(&self) -> usize {
            25
        }
    }

    struct PanelOutlierData;

    impl model::DataSource for PanelOutlierData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "time" => Some((0..25).map(|index| index as f64 * 0.25).collect()),
                "signal" => Some((0..25).map(|index| (index as f64 * 0.25).sin()).collect()),
                "outlier" => Some(vec![10_000.0; 25]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            ["time", "signal", "outlier"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        }

        fn len(&self) -> usize {
            25
        }
    }

    struct ReorderedData;

    impl model::DataSource for ReorderedData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "signal" => Some(vec![3.0, 4.0]),
                "time" => Some(vec![1.0, 2.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            vec!["signal".to_owned(), "time".to_owned()]
        }

        fn len(&self) -> usize {
            2
        }
    }

    struct LargeOffsetData;

    impl model::DataSource for LargeOffsetData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "x" => Some(vec![1_000_000_000_000.0, 1_000_000_000_000.001]),
                "y" => Some(vec![2.0, 3.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            vec!["x".to_owned(), "y".to_owned()]
        }

        fn len(&self) -> usize {
            2
        }
    }

    struct ErrorBarData;

    impl model::DataSource for ErrorBarData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "x" => Some(vec![10.0, 20.0, 30.0]),
                "y" => Some(vec![10.0, 20.0, 30.0]),
                "dx" => Some(vec![2.0, 3.0, 4.0]),
                "dy" => Some(vec![1.0, 2.0, 3.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            ["x", "y", "dx", "dy"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        }

        fn len(&self) -> usize {
            3
        }
    }

    struct NegativeErrorData;

    impl model::DataSource for NegativeErrorData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "x" => Some(vec![10.0, 20.0, 30.0]),
                "y" => Some(vec![10.0, 20.0, 30.0]),
                "dy" => Some(vec![1.0, -2.0, 3.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            ["x", "y", "dy"].into_iter().map(str::to_owned).collect()
        }

        fn len(&self) -> usize {
            3
        }
    }

    struct LogErrorData;

    impl model::DataSource for LogErrorData {
        fn column(&self, name: &str) -> Option<Vec<f64>> {
            match name {
                "x" | "y" => Some(vec![1.0, 10.0]),
                "dx" | "dy" => Some(vec![2.0, 1.0]),
                _ => None,
            }
        }

        fn column_names(&self) -> Vec<String> {
            ["x", "y", "dx", "dy"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        }

        fn len(&self) -> usize {
            2
        }
    }

    fn demo_fixture() -> EditorFixture {
        let mut fixture = EditorFixture {
            controller: PlotController::new(),
            view: PlotEditorView::new(),
        };
        fixture
            .controller
            .load_from_data_source(&SnapshotData)
            .expect("fixture data should load");
        fixture
    }

    fn error_bar_fixture() -> EditorFixture {
        let mut fixture = EditorFixture {
            controller: PlotController::new(),
            view: PlotEditorView::new(),
        };
        fixture
            .controller
            .load_from_data_source(&ErrorBarData)
            .expect("error bar fixture data should load");
        let series_id = fixture.controller.model.series[0].id;
        for (axis, column) in [(model::AxisKind::X, "dx"), (model::AxisKind::Y, "dy")] {
            fixture
                .controller
                .dispatch(controller::action::Action::SetSeriesErrorColumn {
                    series_id,
                    axis,
                    column: Some(column.to_owned()),
                })
                .unwrap();
        }
        fixture
    }

    #[test]
    fn data_source_preserves_f64_points_with_large_shared_offsets() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&LargeOffsetData)
            .expect("fixture data should load");
        let series_id = controller.model.series[0].id;
        let points = controller
            .points_for_series(series_id)
            .expect("series points should be available");

        assert_eq!(points[0], (1_000_000_000_000.0, 2.0));
        assert_eq!(points[1], (1_000_000_000_000.001, 3.0));
        assert_ne!(points[0].0, points[1].0);
        let x_range = view::series::resolve_data_range(
            &model::RangePolicy::Auto,
            points.iter().copied(),
            true,
            -1.0..1.0,
            model::ScaleType::Linear,
        );
        assert!(x_range.start < points[0].0 && x_range.end > points[1].0);
        assert!(x_range.end - x_range.start < 0.01);
    }

    #[test]
    fn manual_log_ranges_are_transformed_to_plot_coordinates() {
        let range = view::series::resolve_data_range(
            &model::RangePolicy::Manual {
                min: 1.0,
                max: 100.0,
            },
            std::iter::empty(),
            true,
            -1.0..1.0,
            model::ScaleType::Log10,
        );
        assert_eq!(range, 0.0..2.0);
    }

    #[test]
    fn manual_axis_range_suggestion_uses_visible_positive_log_data() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&SnapshotData)
            .expect("fixture data should load");
        controller
            .dispatch(controller::action::Action::SetAxisScale {
                axis: model::AxisKind::X,
                scale: model::ScaleType::Log10,
            })
            .unwrap();
        let (min, max) = controller
            .suggested_axis_range(model::AxisKind::X)
            .expect("visible positive values should have a log range");
        assert!(min > 0.0 && min < max);
    }

    #[test]
    fn reimport_preserves_named_series_bindings_and_clears_old_undo_history() {
        let mut fixture = demo_fixture();
        fixture.controller.model.series[0].x_column = "signal".to_owned();
        fixture.controller.model.series[0].y_column = "time".to_owned();
        fixture
            .controller
            .dispatch(controller::action::Action::SetChartTitle(
                "Old table edit".to_owned(),
            ))
            .unwrap();

        fixture
            .controller
            .load_from_data_source(&ReorderedData)
            .expect("reordered data should load");
        assert_eq!(fixture.controller.model.series[0].x_column, "signal");
        assert_eq!(fixture.controller.model.series[0].y_column, "time");
        assert!(fixture.controller.undo_stack.is_empty());
        assert!(fixture.controller.redo_stack.is_empty());

        fixture
            .controller
            .load_from_data_source(&LargeOffsetData)
            .expect("table with missing binding should still load");
        assert_eq!(fixture.controller.model.series[0].x_column, "signal");
        assert!(
            fixture
                .controller
                .notification()
                .is_some_and(|notification| notification
                    .message
                    .contains("missing series columns"))
        );
        assert!(
            fixture
                .controller
                .points_for_series(fixture.controller.model.series[0].id)
                .is_err()
        );
    }

    #[test]
    fn uncertainty_columns_produce_scaled_x_and_y_error_bars() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&ErrorBarData)
            .expect("fixture data should load");
        let series_id = controller.model.series[0].id;
        for (axis, column) in [(model::AxisKind::X, "dx"), (model::AxisKind::Y, "dy")] {
            controller
                .dispatch(controller::action::Action::SetSeriesErrorColumn {
                    series_id,
                    axis,
                    column: Some(column.to_owned()),
                })
                .unwrap();
        }
        assert_eq!(
            controller.model.series[0].y_error_column.as_deref(),
            Some("dy")
        );
        let (x_min, x_max) = controller.suggested_axis_range(model::AxisKind::X).unwrap();
        let (y_min, y_max) = controller.suggested_axis_range(model::AxisKind::Y).unwrap();
        assert!(x_min < 8.0 && x_max > 34.0, "suggestion: {x_min}..{x_max}");
        assert!(y_min < 9.0 && y_max > 33.0, "suggestion: {y_min}..{y_max}");
        controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(controller.model.series[0].y_error_column.is_none());
        controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        controller
            .dispatch(controller::action::Action::SetSeriesAreaBaseline {
                series_id,
                baseline: Some(-5.0),
            })
            .unwrap();
        let (y_min, _) = controller.suggested_axis_range(model::AxisKind::Y).unwrap();
        assert!(y_min < -5.0);
        controller
            .dispatch(controller::action::Action::SetAxisScale {
                axis: model::AxisKind::X,
                scale: model::ScaleType::Log10,
            })
            .unwrap();
        controller
            .dispatch(controller::action::Action::SetAxisScale {
                axis: model::AxisKind::Y,
                scale: model::ScaleType::LogE,
            })
            .unwrap();

        let bars = controller.error_bars_for_series(series_id).unwrap();
        assert_eq!(bars.bars.len(), 3);
        assert_eq!(bars.bars[0].center, (1.0, 10.0_f64.ln()));
        let (x_low, x_high) = bars.bars[0].x_bounds.unwrap();
        assert!((x_low - 8.0_f64.log10()).abs() < 1e-12);
        assert!((x_high - 12.0_f64.log10()).abs() < 1e-12);
        let (y_low, y_high) = bars.bars[0].y_bounds.unwrap();
        assert!((y_low - 9.0_f64.ln()).abs() < 1e-12);
        assert!((y_high - 11.0_f64.ln()).abs() < 1e-12);
        assert!(
            bars.bars[0]
                .range_points()
                .any(|(x, _)| (x - x_low).abs() < 1e-12)
        );

        let path =
            std::env::temp_dir().join(format!("kithe_plot_error_bars_{}.svg", std::process::id()));
        controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("error bar SVG should be readable");
        std::fs::remove_file(path).expect("temporary SVG should be removed");
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn negative_uncertainties_are_rejected_without_flipping_or_replacing_data() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&ErrorBarData)
            .expect("valid fixture should load");
        let series_id = controller.model.series[0].id;
        controller
            .dispatch(controller::action::Action::SetSeriesErrorColumn {
                series_id,
                axis: model::AxisKind::Y,
                column: Some("dy".to_owned()),
            })
            .unwrap();
        let original_points = controller.points_for_series(series_id).unwrap();

        let result = controller.load_from_data_source(&NegativeErrorData);
        assert!(result.is_err());
        assert_eq!(
            controller.points_for_series(series_id).unwrap(),
            original_points
        );

        let mut unconfigured = PlotController::new();
        unconfigured
            .load_from_data_source(&NegativeErrorData)
            .expect("an unused negative-valued column is ordinary data");
        let series_id = unconfigured.model.series[0].id;
        let result = unconfigured.dispatch(controller::action::Action::SetSeriesErrorColumn {
            series_id,
            axis: model::AxisKind::Y,
            column: Some("dy".to_owned()),
        });
        assert!(result.is_err());
        assert!(unconfigured.model.series[0].y_error_column.is_none());
        let message = unconfigured
            .notification()
            .map(|notification| notification.message.clone())
            .unwrap_or_default();
        assert!(
            message.contains("negative magnitude") && message.contains("row 2"),
            "unexpected validation message: {message}"
        );

        let mut project_model = demo_fixture().controller.model;
        project_model.series[0].y_error_column = Some("dy".to_owned());
        let project = model::PlotProject::new(
            project_model,
            model::DataTable {
                columns: vec![
                    model::ColumnData {
                        name: "time".to_owned(),
                        values: vec![0.0, 1.0, 2.0],
                    },
                    model::ColumnData {
                        name: "signal".to_owned(),
                        values: vec![1.0, 2.0, 3.0],
                    },
                    model::ColumnData {
                        name: "dy".to_owned(),
                        values: vec![1.0, -2.0, 3.0],
                    },
                ],
                row_count: 3,
            },
            None,
            "fixture.csv".to_owned(),
            model::PublicationDimensions::default(),
        );
        assert!(project.validate().is_err());
    }

    #[test]
    fn asymmetric_uncertainty_columns_create_one_sided_bounds_and_are_undoable() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&ErrorBarData)
            .expect("fixture data should load");
        let series_id = controller.model.series[0].id;
        controller
            .dispatch(controller::action::Action::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis: model::AxisKind::X,
                side: model::ErrorBarSide::Lower,
                column: Some("dx".to_owned()),
            })
            .unwrap();
        controller
            .dispatch(controller::action::Action::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis: model::AxisKind::Y,
                side: model::ErrorBarSide::Upper,
                column: Some("dy".to_owned()),
            })
            .unwrap();

        let bars = controller.error_bars_for_series(series_id).unwrap();
        assert_eq!(bars.bars[0].x_bounds, Some((8.0, 10.0)));
        assert_eq!(bars.bars[0].y_bounds, Some((10.0, 11.0)));
        controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(controller.model.series[0].y_error_upper_column.is_none());
        assert_eq!(
            controller.model.series[0].x_error_lower_column.as_deref(),
            Some("dx")
        );
        controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(
            controller.model.series[0].y_error_upper_column.as_deref(),
            Some("dy")
        );
    }

    #[test]
    fn asymmetric_uncertainty_mode_disables_inactive_symmetric_selector() {
        let mut fixture = error_bar_fixture();
        let series_id = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesAsymmetricErrorColumn {
                series_id,
                axis: model::AxisKind::X,
                side: model::ErrorBarSide::Lower,
                column: Some("dx".to_owned()),
            })
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness.get_by_label_contains("Series 1 | x -> y").click();
        harness.run_ok();
        harness.get_by_label("X intervals: asymmetric");
        harness.get_by_label("Y intervals: symmetric");
        assert!(
            harness
                .get_by_label("X symmetric uncertainty")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            !harness
                .get_by_label("Y symmetric uncertainty")
                .accesskit_node()
                .is_disabled()
        );
    }

    #[test]
    fn log_scale_reports_error_intervals_omitted_at_non_positive_endpoints() {
        let mut fixture = EditorFixture {
            controller: PlotController::new(),
            view: PlotEditorView::new(),
        };
        fixture
            .controller
            .load_from_data_source(&LogErrorData)
            .unwrap();
        fixture.controller.model.axes.x.scale = model::ScaleType::Log10Axis;
        fixture.controller.model.axes.y.scale = model::ScaleType::Log10;
        let series_id = fixture.controller.model.series[0].id;
        for axis in [model::AxisKind::X, model::AxisKind::Y] {
            fixture
                .controller
                .dispatch(controller::action::Action::SetSeriesErrorColumn {
                    series_id,
                    axis,
                    column: Some(
                        if axis == model::AxisKind::X {
                            "dx"
                        } else {
                            "dy"
                        }
                        .to_owned(),
                    ),
                })
                .unwrap();
        }
        let intervals = fixture.controller.error_bars_for_series(series_id).unwrap();
        assert_eq!((intervals.omitted_log_x, intervals.omitted_log_y), (1, 1));
        assert_eq!(intervals.bars.len(), 1);

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness.get_by_label_contains("Series 1 | x -> y").click();
        harness.run_ok();
        harness.get_by_label(
            "Series 1: log scale omits error intervals (X: 1, Y: 1) because an endpoint is non-positive",
        );
        let with_intervals =
            std::env::temp_dir().join(format!("kithe_plot_log_errors_{}.svg", std::process::id()));
        let without_intervals = std::env::temp_dir().join(format!(
            "kithe_plot_log_errors_none_{}.svg",
            std::process::id()
        ));
        for path in [&with_intervals, &without_intervals] {
            harness
                .state_mut()
                .controller
                .dispatch(controller::action::Action::RequestSaveAs {
                    path: path.to_string_lossy().into_owned(),
                    format: model::ImageFormat::Svg,
                    size: model::ImageSize {
                        width: 800,
                        height: 600,
                    },
                })
                .unwrap();
            if path == &with_intervals {
                for axis in [model::AxisKind::X, model::AxisKind::Y] {
                    harness
                        .state_mut()
                        .controller
                        .dispatch(controller::action::Action::SetSeriesErrorColumn {
                            series_id,
                            axis,
                            column: None,
                        })
                        .unwrap();
                }
            }
        }
        let with_svg = std::fs::read_to_string(&with_intervals).unwrap();
        let without_svg = std::fs::read_to_string(&without_intervals).unwrap();
        assert_ne!(with_svg, without_svg);
        std::fs::remove_file(with_intervals).unwrap();
        std::fs::remove_file(without_intervals).unwrap();
    }

    #[test]
    fn invalid_log_points_split_lines_into_separate_segments() {
        let (points, segments) = controller::scale_points_into_segments(
            &[(1.0, 1.0), (2.0, 0.0), (3.0, 100.0)],
            model::ScaleType::Linear,
            model::ScaleType::Log10,
        );
        assert_eq!(points, vec![(1.0, 0.0), (3.0, 2.0)]);
        assert_eq!(segments, vec![vec![(1.0, 0.0)], vec![(3.0, 2.0)]]);
    }

    #[test]
    fn preview_renders_series_with_uncertainty_columns() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                error_bar_fixture(),
            );
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.series[0]
                .x_error_column
                .as_deref(),
            Some("dx")
        );
        assert_eq!(
            harness.state().controller.model.series[0]
                .y_error_column
                .as_deref(),
            Some("dy")
        );
    }

    #[test]
    fn native_project_round_trip_embeds_data_and_plot_configuration() {
        let custom_family = format!("Project Font {}", std::process::id());
        let custom_asset_id = register_chart_font(
            &egui::Context::default(),
            custom_family.clone(),
            include_bytes!("../assets/fonts/NotoSerif.ttf").to_vec(),
        )
        .expect("project font should register");
        let mut original = PlotController::new();
        original
            .load_from_data_source(&ErrorBarData)
            .expect("fixture data should load");
        original.model.layout.font_family = model::PlotFontFamily::CustomAsset {
            family: custom_family.clone(),
            asset_id: custom_asset_id.clone(),
        };
        let series_id = original.model.series[0].id;
        original
            .dispatch(controller::action::Action::SetChartTitle(
                "Portable experiment".to_owned(),
            ))
            .unwrap();
        original
            .dispatch(controller::action::Action::SetAxisTitleAnchor {
                axis: model::AxisKind::X,
                anchor: Some([0.35, 0.7]),
            })
            .unwrap();
        original
            .dispatch(controller::action::Action::SetSeriesErrorColumn {
                series_id,
                axis: model::AxisKind::Y,
                column: Some("dy".to_owned()),
            })
            .unwrap();
        original.model.series[0].x_error_lower_column = Some("dx".to_owned());
        original.model.series[0].y_error_upper_column = Some("dy".to_owned());
        original
            .dispatch(controller::action::Action::SetSeriesMarker {
                series_id,
                marker: Some(model::MarkerShape::Triangle),
                size: 6.0,
            })
            .unwrap();
        original
            .dispatch(controller::action::Action::SetAnnotations(vec![
                model::PlotAnnotation {
                    id: 17,
                    text: "peak".to_owned(),
                    x: 20.0,
                    y: 22.0,
                    font_size: 14,
                    color: model::Color {
                        r: 20,
                        g: 80,
                        b: 160,
                        a: 220,
                    },
                },
            ]))
            .unwrap();
        original
            .dispatch(controller::action::Action::SetPanels(vec![
                model::PlotPanel {
                    id: 9,
                    title: "Measurement".to_owned(),
                    series_ids: vec![series_id],
                },
            ]))
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "kithe_plot_round_trip_{}.kplot",
            std::process::id()
        ));
        let publication_dimensions = model::PublicationDimensions {
            width: 85.0,
            height: 55.0,
            unit: model::PhysicalUnit::Millimeters,
            dpi: 600,
        };
        original
            .dispatch(controller::action::Action::SaveProject {
                path: path.to_string_lossy().into_owned(),
                dimensions: publication_dimensions,
            })
            .unwrap();

        let mut loaded = PlotController::new();
        loaded
            .dispatch(controller::action::Action::OpenProject {
                path: path.to_string_lossy().into_owned(),
            })
            .unwrap();
        std::fs::remove_file(path).expect("temporary project should be removed");

        assert!(loaded.has_data());
        assert!(loaded.undo_stack.is_empty());
        assert_eq!(
            loaded.project_publication_dimensions(),
            Some(publication_dimensions)
        );
        assert_eq!(loaded.data_summary().unwrap().source, "Host-provided data");
        assert_eq!(loaded.model.layout.title, "Portable experiment");
        assert!(
            loaded.model == original.model,
            "plot model changed across save/open round trip"
        );
        assert_eq!(loaded.model.axes.x.title_anchor, Some([0.35, 0.7]));
        assert_eq!(
            loaded.model.layout.font_family,
            model::PlotFontFamily::CustomAsset {
                family: custom_family,
                asset_id: custom_asset_id,
            }
        );
        assert_eq!(loaded.model.series[0].y_error_column.as_deref(), Some("dy"));
        assert_eq!(
            loaded.model.series[0].x_error_lower_column.as_deref(),
            Some("dx")
        );
        assert_eq!(
            loaded.model.series[0].y_error_upper_column.as_deref(),
            Some("dy")
        );
        assert_eq!(
            loaded.model.series[0]
                .style
                .marker
                .as_ref()
                .map(|marker| (marker.shape, marker.size)),
            Some((model::MarkerShape::Triangle, 6.0))
        );
        assert_eq!(loaded.available_columns(), vec!["x", "y", "dx", "dy"]);
        assert_eq!(
            loaded.points_for_series(series_id).unwrap(),
            original.points_for_series(series_id).unwrap()
        );
        assert_eq!(
            loaded.points_for_series(series_id).unwrap()[1],
            (20.0, 20.0)
        );
        let svg_path =
            std::env::temp_dir().join(format!("kithe_plot_round_trip_{}.svg", std::process::id()));
        loaded
            .dispatch(controller::action::Action::RequestPublicationExport {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                dimensions: publication_dimensions,
            })
            .unwrap();
        let svg = std::fs::read_to_string(&svg_path).expect("round-trip SVG should be readable");
        std::fs::remove_file(svg_path).expect("temporary round-trip SVG should be removed");
        assert!(svg.contains("width=\"85.0000mm\""));
        assert!(svg.contains("height=\"55.0000mm\""));
        assert!(
            svg.contains("viewBox=\"0 0 2008 1299\""),
            "unexpected SVG root: {}",
            &svg[..svg.find('>').unwrap_or(svg.len()).min(250)]
        );

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                EditorFixture {
                    controller: loaded,
                    view: PlotEditorView::new(),
                },
            );
        harness.run_ok();
        assert_eq!(
            harness.state().view.export_dimensions(),
            publication_dimensions
        );
    }

    #[test]
    fn projects_keep_same_named_font_assets_independent() {
        let family = format!("Shared Lab Font {}", std::process::id());
        let ctx = egui::Context::default();
        let sans_id = register_chart_font(
            &ctx,
            family.clone(),
            include_bytes!("../assets/fonts/NotoSans.ttf").to_vec(),
        )
        .expect("first font should register");
        let serif_id = register_chart_font(
            &ctx,
            family.clone(),
            include_bytes!("../assets/fonts/NotoSerif.ttf").to_vec(),
        )
        .expect("second font with the same family name should register");
        assert_ne!(sans_id, serif_id);

        let mut stored_ids = Vec::new();
        for (label, asset_id, expected_font) in [
            (
                "sans",
                sans_id.clone(),
                include_bytes!("../assets/fonts/NotoSans.ttf").as_slice(),
            ),
            (
                "serif",
                serif_id.clone(),
                include_bytes!("../assets/fonts/NotoSerif.ttf").as_slice(),
            ),
        ] {
            let mut controller = PlotController::new();
            controller
                .load_from_data_source(&ErrorBarData)
                .expect("fixture data should load");
            controller.model.layout.font_family = model::PlotFontFamily::CustomAsset {
                family: family.clone(),
                asset_id: asset_id.clone(),
            };
            let path = std::env::temp_dir().join(format!(
                "kithe_plot_same_name_{label}_{}.kplot",
                std::process::id()
            ));
            controller
                .dispatch(controller::action::Action::SaveProject {
                    path: path.to_string_lossy().into_owned(),
                    dimensions: model::PublicationDimensions::default(),
                })
                .expect("project should save");

            let mut reopened = PlotController::new();
            reopened
                .dispatch(controller::action::Action::OpenProject {
                    path: path.to_string_lossy().into_owned(),
                })
                .expect("project should reopen");
            std::fs::remove_file(path).expect("project fixture should be removed");
            assert_eq!(
                reopened.model.layout.font_family,
                model::PlotFontFamily::CustomAsset {
                    family: family.clone(),
                    asset_id: asset_id.clone(),
                }
            );
            let svg_path = std::env::temp_dir().join(format!(
                "kithe_plot_same_name_{label}_{}.svg",
                std::process::id()
            ));
            reopened
                .dispatch(controller::action::Action::RequestSaveAs {
                    path: svg_path.to_string_lossy().into_owned(),
                    format: model::ImageFormat::Svg,
                    size: model::ImageSize {
                        width: 600,
                        height: 400,
                    },
                })
                .expect("reopened font should render to SVG");
            let svg = std::fs::read_to_string(&svg_path).expect("SVG should be readable");
            std::fs::remove_file(svg_path).expect("SVG fixture should be removed");
            assert!(svg.contains(&format!("font-family:'KiThePlot Custom {asset_id}'")));
            let embedded_font =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, expected_font);
            assert!(svg.contains(&embedded_font));
            stored_ids.push(asset_id);
        }
        assert_eq!(stored_ids, vec![sans_id, serif_id]);
    }

    #[test]
    fn version_one_custom_font_projects_migrate_to_fingerprinted_references() {
        let mut fixture = demo_fixture();
        let family = format!("Legacy Project Font {}", std::process::id());
        let bytes = include_bytes!("../assets/fonts/NotoSans.ttf");
        let mut project = model::PlotProject::new(
            fixture.controller.model.clone(),
            model::DataTable::from_data_source(&SnapshotData).unwrap(),
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                bytes,
            )),
            "snapshot.csv".to_owned(),
            model::PublicationDimensions::default(),
        );
        project.format_version = 1;
        project.model.layout.font_family = model::PlotFontFamily::Custom(family.clone());
        let mut json = serde_json::to_value(project).unwrap();
        json["model"]["layout"]["font_family"] = serde_json::json!({ "Custom": family });
        let path = std::env::temp_dir().join(format!(
            "kithe_plot_legacy_font_{}.kplot",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(&json).unwrap())
            .expect("legacy project should be written");
        fixture
            .controller
            .dispatch(controller::action::Action::OpenProject {
                path: path.to_string_lossy().into_owned(),
            })
            .expect("version 1 custom-font project should migrate");
        std::fs::remove_file(path).expect("legacy project fixture should be removed");
        assert_eq!(
            fixture.controller.model.layout.font_family,
            model::PlotFontFamily::CustomAsset {
                family,
                asset_id: crate::font::font_asset_id(bytes),
            }
        );
    }

    #[test]
    fn import_preview_keeps_existing_data_until_a_complete_valid_import_is_confirmed() {
        let mut controller = PlotController::new();
        controller
            .load_from_data_source(&SnapshotData)
            .expect("initial fixture should load");
        let original_columns = controller.available_columns();
        let path = std::env::temp_dir().join(format!(
            "kithe_plot_import_preview_{}.csv",
            std::process::id()
        ));
        std::fs::write(&path, "x;y\n1;2\n3;4\n").expect("valid fixture should be written");
        let options = model::ImportOptions {
            delimiter: model::ImportDelimiter::Semicolon,
            decimal: model::DecimalConvention::Dot,
            header: model::HeaderMode::Present,
        };
        controller
            .dispatch(controller::action::Action::PreviewImport {
                path: path.to_string_lossy().into_owned(),
                options,
            })
            .unwrap();
        assert_eq!(controller.available_columns(), original_columns);
        assert_eq!(controller.import_preview().unwrap().2.accepted_rows, 2);
        controller
            .dispatch(controller::action::Action::ConfirmImport)
            .unwrap();
        assert_eq!(controller.available_columns(), vec!["x", "y"]);
        let summary = controller.data_summary().expect("loaded table summary");
        assert_eq!(summary.source, path.file_name().unwrap().to_string_lossy());
        assert_eq!((summary.rows, summary.columns), (2, 2));

        std::fs::write(&path, "x;y\n5;6\n7;invalid\n").expect("invalid fixture should be written");
        controller
            .dispatch(controller::action::Action::PreviewImport {
                path: path.to_string_lossy().into_owned(),
                options,
            })
            .unwrap();
        assert_eq!(controller.import_preview().unwrap().2.rejected_rows, 1);
        assert!(
            controller
                .dispatch(controller::action::Action::ConfirmImport)
                .is_err()
        );
        assert_eq!(controller.available_columns(), vec!["x", "y"]);
        std::fs::remove_file(path).expect("import fixture should be removed");
    }

    #[test]
    fn import_preview_dialog_snapshot_and_cancel_leave_the_dataset_unchanged() {
        let mut fixture = demo_fixture();
        let original_columns = fixture.controller.available_columns();
        let fixture_dir =
            std::env::temp_dir().join(format!("kithe_plot_import_dialog_{}", std::process::id()));
        std::fs::create_dir_all(&fixture_dir).expect("fixture directory should be created");
        let path = fixture_dir.join("import.csv");
        std::fs::write(&path, "time,signal\n0,1\n1,2\n")
            .expect("preview fixture should be written");
        fixture
            .controller
            .dispatch(controller::action::Action::PreviewImport {
                path: path.to_string_lossy().into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot("import_preview_dialog");
        harness.get_by_label("Cancel").click();
        harness.run_ok();
        assert!(harness.state().controller.import_preview().is_none());
        assert_eq!(
            harness.state().controller.available_columns(),
            original_columns
        );
        std::fs::remove_file(path).expect("preview fixture should be removed");
        std::fs::remove_dir(fixture_dir).expect("fixture directory should be removed");
    }

    #[test]
    fn plot_wheel_zoom_updates_saved_axis_ranges_and_is_undoable() {
        let fixture = demo_fixture();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        let before = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::X)
            .unwrap();
        harness.run();
        let pointer = egui::pos2(180.0, 340.0);
        harness.hover_at(pointer);
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -80.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();

        let controller = &harness.state().controller;
        let RangePolicyForTest::Manual(after_min, after_max) =
            RangePolicyForTest::from(&controller.model.axes.x.range)
        else {
            panic!("wheel zoom should persist the visible X range");
        };
        assert!(after_max - after_min < before.1 - before.0);
        assert_eq!(controller.undo_stack.len(), 1);
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(matches!(
            harness.state().controller.model.axes.x.range,
            model::RangePolicy::Auto
        ));

        let before_pan = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::X)
            .unwrap();
        let destination = pointer + egui::vec2(24.0, 12.0);
        harness.hover_at(pointer);
        harness.drag_at(pointer);
        harness.run();
        for position in [
            pointer + egui::vec2(8.0, 4.0),
            pointer + egui::vec2(16.0, 8.0),
            destination,
        ] {
            harness.hover_at(position);
            harness.run_ok();
        }
        harness.drop_at(destination);
        harness.run();
        let pan_range = &harness.state().controller.model.axes.x.range;
        assert!(matches!(pan_range, model::RangePolicy::Manual { .. }));
        assert_eq!(harness.state().controller.undo_stack.len(), 1);
        if let model::RangePolicy::Manual { min, max } = pan_range {
            assert!((*min - before_pan.0).abs() > f64::EPSILON);
            assert!((*max - *min - (before_pan.1 - before_pan.0)).abs() < 1e-8);
        }
        let saved_range = match &harness.state().controller.model.axes.x.range {
            model::RangePolicy::Manual { min, max } => (*min, *max),
            model::RangePolicy::Auto => panic!("pan should leave a manual view range"),
        };
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(matches!(
            harness.state().controller.model.axes.x.range,
            model::RangePolicy::Auto
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert!(matches!(
            harness.state().controller.model.axes.x.range,
            model::RangePolicy::Manual { min, max }
                if min == saved_range.0 && max == saved_range.1
        ));
        let project_path = std::env::temp_dir().join(format!(
            "kithe_plot_saved_view_{}.kplot",
            std::process::id()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();
        let mut reopened = PlotController::new();
        reopened
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        std::fs::remove_file(project_path).expect("saved-view fixture should be removed");
        assert!(matches!(
            reopened.model.axes.x.range,
            model::RangePolicy::Manual { min, max }
                if min == saved_range.0 && max == saved_range.1
        ));
    }

    enum RangePolicyForTest {
        Manual(f64, f64),
        Auto,
    }

    impl From<&model::RangePolicy> for RangePolicyForTest {
        fn from(value: &model::RangePolicy) -> Self {
            match value {
                model::RangePolicy::Manual { min, max } => Self::Manual(*min, *max),
                model::RangePolicy::Auto => Self::Auto,
            }
        }
    }

    #[test]
    fn embeddable_app_accepts_host_plot_defaults_before_data_load() {
        let mut model = model::PlotModel::default();
        model.layout.title = "Host default".to_owned();
        model.layout.font_family = model::PlotFontFamily::Monospace;
        model.series[0].style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 5.0,
        });
        let mut app = PlotEditorApp::with_model(model);
        app.controller_mut()
            .load_from_data_source(&SnapshotData)
            .expect("host fixture data should load");

        let configured = &app.controller_mut().model;
        assert_eq!(configured.layout.title, "Host default");
        assert_eq!(
            configured.layout.font_family,
            model::PlotFontFamily::Monospace
        );
        assert_eq!(configured.series[0].x_column, "time");
        assert_eq!(
            configured.series[0]
                .style
                .marker
                .as_ref()
                .map(|marker| marker.shape),
            Some(model::MarkerShape::Square)
        );
    }

    #[test]
    fn invalid_project_does_not_replace_loaded_data() {
        let mut controller = demo_fixture().controller;
        let old_title = controller.model.layout.title.clone();
        let path = std::env::temp_dir().join(format!(
            "kithe_plot_invalid_project_{}.kplot",
            std::process::id()
        ));
        std::fs::write(&path, "{ invalid json }").expect("temporary project should be written");
        let result = controller.dispatch(controller::action::Action::OpenProject {
            path: path.to_string_lossy().into_owned(),
        });
        std::fs::remove_file(path).expect("temporary project should be removed");

        assert!(result.is_err());
        assert!(controller.has_data());
        assert_eq!(controller.model.layout.title, old_title);
    }

    #[test]
    fn failed_export_keeps_previous_destination_file_intact() {
        let path = std::env::temp_dir().join(format!(
            "kithe_plot_failed_export_{}.png",
            std::process::id()
        ));
        std::fs::write(&path, b"previous valid image").expect("fixture should be written");
        let mut controller = PlotController::new();
        let result = controller.dispatch(controller::action::Action::RequestSaveAs {
            path: path.to_string_lossy().into_owned(),
            format: model::ImageFormat::Png,
            size: model::ImageSize {
                width: 640,
                height: 480,
            },
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous valid image");
        std::fs::remove_file(path).expect("fixture should be removed");
    }

    #[test]
    fn title_visibility_control_updates_plot_model() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );

        harness.get_by_label("Chart title & typography").click();
        harness.run_ok();
        harness.get_by_label("Show title").click();
        harness.run_ok();
        assert!(!harness.state().controller.model.layout.title_visible);
    }

    #[test]
    fn conventional_log_axis_can_be_selected_in_the_editor() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );

        harness.get_by_label("X Axis").click();
        harness.run_ok();
        harness.get_by_label("Scale").click();
        harness.run_ok();
        harness.get_by_label("Log10 axis (1, 10, 100)").click();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.axes.x.scale,
            model::ScaleType::Log10Axis
        );
    }

    #[test]
    fn clicking_a_plotted_point_shows_its_source_row_and_values() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );

        harness.run();
        let series_id = harness.state().controller.model.series[0].id;
        let points = harness
            .state()
            .controller
            .points_for_series(series_id)
            .unwrap();
        let data_rect = harness.state().view.last_data_rect().unwrap();
        let (x_min, x_max) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), point| {
                (min.min(point.0), max.max(point.0))
            });
        let (y_min, y_max) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), point| {
                (min.min(point.1), max.max(point.1))
            });
        let x_range = (x_min - (x_max - x_min) * 0.05)..(x_max + (x_max - x_min) * 0.05);
        let y_range = (y_min - (y_max - y_min) * 0.05)..(y_max + (y_max - y_min) * 0.05);
        let (x, y) = points[12];
        let click_position = egui::pos2(
            data_rect.left()
                + ((x - x_range.start) / (x_range.end - x_range.start)) as f32 * data_rect.width(),
            data_rect.bottom()
                - ((y - y_range.start) / (y_range.end - y_range.start)) as f32 * data_rect.height(),
        );
        harness.event(egui::Event::PointerMoved(click_position));
        harness.event(egui::Event::PointerButton {
            pos: click_position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.event(egui::Event::PointerButton {
            pos: click_position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
        assert_eq!(harness.state().view.selected_series(), Some(series_id));
        assert_eq!(
            harness.state().view.inspected_point(),
            Some((series_id, 12))
        );
        harness.get_by_label_contains("Selected series: Series 1");
        harness.get_by_label("Dataset row: 13");
        harness.get_by_label("X: time = 3");
        harness.get_by_label("Y: signal = 0.1411200080598672");

        let import_path = std::env::temp_dir().join(format!(
            "kithe_plot_inspector_reimport_{}.csv",
            std::process::id()
        ));
        std::fs::write(&import_path, "time,signal\n1,7\n2,8\n")
            .expect("replacement table should be written");
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::PreviewImport {
                path: import_path.to_string_lossy().into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::ConfirmImport)
            .unwrap();
        harness.run_ok();
        assert_eq!(harness.state().view.selected_series(), None);
        assert_eq!(harness.state().view.inspected_point(), None);
        std::fs::remove_file(import_path).expect("temporary replacement table should be removed");
    }

    #[test]
    fn clicking_between_sparse_points_selects_the_curve() {
        let mut fixture = error_bar_fixture();
        fixture.controller.model.axes.x.range = model::RangePolicy::Manual {
            min: 11.0,
            max: 19.0,
        };
        fixture.controller.model.axes.y.range = model::RangePolicy::Manual {
            min: 11.0,
            max: 19.0,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );

        harness.run();
        let series_id = harness.state().controller.model.series[0].id;
        let data_rect = harness.state().view.last_data_rect().unwrap();
        let x_range = 11.0..19.0;
        let y_range = 11.0..19.0;
        let to_screen = |x: f64, y: f64| {
            egui::pos2(
                data_rect.left()
                    + ((x - x_range.start) / (x_range.end - x_range.start)) as f32
                        * data_rect.width(),
                data_rect.bottom()
                    - ((y - y_range.start) / (y_range.end - y_range.start)) as f32
                        * data_rect.height(),
            )
        };
        let start = to_screen(10.0, 10.0);
        let end = to_screen(20.0, 20.0);
        let click_position = start + (end - start) * 0.5;
        harness.event(egui::Event::PointerMoved(click_position));
        harness.event(egui::Event::PointerButton {
            pos: click_position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.event(egui::Event::PointerButton {
            pos: click_position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();

        assert_eq!(harness.state().view.selected_series(), Some(series_id));
        assert_eq!(harness.state().view.inspected_point(), None);
        harness.get_by_label("X column: x");
        harness.get_by_label("Y column: y");
    }

    #[test]
    fn clicking_on_visible_step_after_segment_selects_curve() {
        let mut fixture = error_bar_fixture();
        fixture.controller.model.series[0].style.line_interpolation =
            model::LineInterpolation::StepAfter;
        fixture.controller.model.axes.x.range = model::RangePolicy::Manual {
            min: 11.0,
            max: 19.0,
        };
        fixture.controller.model.axes.y.range = model::RangePolicy::Manual {
            min: 9.0,
            max: 21.0,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        let series_id = harness.state().controller.model.series[0].id;
        let data_rect = harness.state().view.last_data_rect().unwrap();
        let point = egui::pos2(
            data_rect.left() + 0.5 * data_rect.width(),
            data_rect.bottom() - (1.0 / 12.0) * data_rect.height(),
        );
        harness.event(egui::Event::PointerMoved(point));
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
        assert_eq!(harness.state().view.selected_series(), Some(series_id));
    }

    #[test]
    fn composite_edit_history_survives_failures_and_project_round_trip() {
        let mut fixture = demo_fixture();
        let initial_model = fixture.controller.model.clone();
        fixture
            .controller
            .dispatch(controller::action::Action::SetChartTitle(
                "Measured response".to_owned(),
            ))
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxesRanges {
                x: model::RangePolicy::Manual { min: 0.5, max: 5.5 },
                y: model::RangePolicy::Manual {
                    min: -0.8,
                    max: 0.8,
                },
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendAnchor([0.32, 0.27]))
            .unwrap();
        let edited_model = fixture.controller.model.clone();

        fixture
            .controller
            .dispatch(controller::action::Action::PreviewImport {
                path: std::env::temp_dir()
                    .join("kitheplot_missing_import_for_undo.csv")
                    .to_string_lossy()
                    .into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();
        assert!(
            fixture
                .controller
                .dispatch(controller::action::Action::ConfirmImport)
                .is_err()
        );
        assert!(fixture.controller.model == edited_model);

        for _ in 0..3 {
            fixture
                .controller
                .dispatch(controller::action::Action::Undo)
                .unwrap();
        }
        assert!(fixture.controller.model == initial_model);
        for _ in 0..3 {
            fixture
                .controller
                .dispatch(controller::action::Action::Redo)
                .unwrap();
        }
        assert!(fixture.controller.model == edited_model);

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project_path = std::env::temp_dir().join(format!("kitheplot_composite_{suffix}.kplot"));
        fixture
            .controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert!(fixture.controller.model == edited_model);

        let blocked_export_dir =
            std::env::temp_dir().join(format!("kitheplot_blocked_{suffix}.png"));
        std::fs::create_dir(&blocked_export_dir).unwrap();
        assert!(
            fixture
                .controller
                .dispatch(controller::action::Action::RequestSaveAs {
                    path: blocked_export_dir.to_string_lossy().into_owned(),
                    format: model::ImageFormat::Png,
                    size: model::ImageSize {
                        width: 320,
                        height: 240,
                    },
                })
                .is_err()
        );
        assert!(fixture.controller.model == edited_model);
        std::fs::remove_file(project_path).unwrap();
        std::fs::remove_dir_all(blocked_export_dir).unwrap();
    }

    #[test]
    fn import_log_scale_project_and_svg_form_one_editor_workflow() {
        let mut fixture = demo_fixture();
        let original_rows = fixture.controller.data_summary().unwrap().rows;
        let dir =
            std::env::temp_dir().join(format!("kithe_plot_user_workflow_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("experiment.csv");
        let project_path = dir.join("experiment.kplot");
        let svg_path = dir.join("experiment.svg");
        std::fs::write(&csv_path, "time,signal\n1,2\n10,5\n100,20\n").unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::PreviewImport {
                path: csv_path.to_string_lossy().into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        assert_eq!(
            harness.state().controller.data_summary().unwrap().rows,
            original_rows,
            "previewing an import must leave the active data unchanged"
        );
        harness.get_by_label("Import data").click();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.data_summary().unwrap().source,
            "experiment.csv"
        );
        assert_eq!(
            harness
                .state()
                .controller
                .points_for_series(harness.state().controller.model.series[0].id)
                .unwrap()
                .len(),
            3
        );

        harness.get_by_label("X Axis").click();
        harness.run_ok();
        harness.get_by_label("Scale").click();
        harness.run_ok();
        harness.get_by_label("Log10 axis (1, 10, 100)").click();
        harness.run_ok();
        let controller = &mut harness.state_mut().controller;
        controller
            .dispatch(controller::action::Action::SetChartTitle(
                "Experiment response".to_owned(),
            ))
            .unwrap();
        controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();

        let mut reopened = PlotController::new();
        reopened
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(reopened.model.axes.x.scale, model::ScaleType::Log10Axis);
        assert_eq!(reopened.model.layout.title, "Experiment response");
        assert_eq!(reopened.data_summary().unwrap().rows, 3);
        reopened
            .dispatch(controller::action::Action::RequestSaveAs {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&svg_path).unwrap();
        assert!(svg.contains("Experiment response"));
        assert!(svg.contains("100"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_measurement_workflow_preserves_row_source_uncertainty_and_export_size() {
        let stem = format!("kithe_plot_workflow_{}", std::process::id());
        let csv_path = std::env::temp_dir().join(format!("{stem}.csv"));
        let project_path = std::env::temp_dir().join(format!("{stem}.kplot"));
        let svg_path = std::env::temp_dir().join(format!("{stem}.svg"));
        std::fs::write(
            &csv_path,
            "time,signal,uncertainty\n0,1,0.1\n1,3,0.2\n2,2,0.15\n",
        )
        .expect("workflow CSV should be written");
        let mut controller = PlotController::new();
        controller
            .dispatch(controller::action::Action::PreviewImport {
                path: csv_path.to_string_lossy().into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();
        controller
            .dispatch(controller::action::Action::ConfirmImport)
            .unwrap();
        let series_id = controller.model.series[0].id;
        controller
            .dispatch(controller::action::Action::SetSeriesErrorColumn {
                series_id,
                axis: model::AxisKind::Y,
                column: Some("uncertainty".to_owned()),
            })
            .unwrap();
        let dimensions = model::PublicationDimensions {
            width: 85.0,
            height: 55.0,
            unit: model::PhysicalUnit::Millimeters,
            dpi: 600,
        };
        controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions,
            })
            .unwrap();

        let mut reopened = PlotController::new();
        reopened
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(
            reopened.data_summary().unwrap().source,
            csv_path.file_name().unwrap().to_string_lossy().into_owned()
        );
        assert_eq!(reopened.project_publication_dimensions(), Some(dimensions));
        assert_eq!(
            reopened.model.series[0].y_error_column.as_deref(),
            Some("uncertainty")
        );

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                EditorFixture {
                    controller: reopened,
                    view: PlotEditorView::new(),
                },
            );
        harness.run();
        let data_rect = harness.state().view.last_data_rect().unwrap();
        let (x_min, x_max) = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::X)
            .unwrap();
        let (y_min, y_max) = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::Y)
            .unwrap();
        let point = egui::pos2(
            data_rect.left() + ((1.0 - x_min) / (x_max - x_min)) as f32 * data_rect.width(),
            data_rect.bottom() - ((3.0 - y_min) / (y_max - y_min)) as f32 * data_rect.height(),
        );
        harness.event(egui::Event::PointerMoved(point));
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_ok();
        assert_eq!(harness.state().view.inspected_point(), Some((series_id, 1)));
        harness.get_by_label("Dataset row: 2");
        harness.get_by_label("Y: signal = 3");
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::RequestPublicationExport {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                dimensions,
            })
            .unwrap();
        let svg = std::fs::read_to_string(&svg_path).expect("workflow SVG should be readable");
        assert!(svg.contains("width=\"85.0000mm\""));
        assert!(svg.contains("height=\"55.0000mm\""));
        assert!(svg.contains("viewBox=\"0 0 2008 1299\""));
        std::fs::remove_file(csv_path).expect("workflow CSV should be removed");
        std::fs::remove_file(project_path).expect("workflow project should be removed");
        std::fs::remove_file(svg_path).expect("workflow SVG should be removed");
    }

    #[test]
    fn reordered_csv_reimport_preserves_series_style_and_binding_through_export() {
        let stem = format!("kithe_plot_reorder_{}", std::process::id());
        let original_csv = std::env::temp_dir().join(format!("{stem}_first.csv"));
        let reordered_csv = std::env::temp_dir().join(format!("{stem}_reordered.csv"));
        let project_path = std::env::temp_dir().join(format!("{stem}.kplot"));
        let svg_path = std::env::temp_dir().join(format!("{stem}.svg"));
        std::fs::write(&original_csv, "time,signal\n0,2\n1,4\n2,6\n").unwrap();
        std::fs::write(&reordered_csv, "signal,time\n5,0\n7,1\n9,2\n").unwrap();

        let mut controller = PlotController::new();
        for path in [&original_csv, &reordered_csv] {
            controller
                .dispatch(controller::action::Action::PreviewImport {
                    path: path.to_string_lossy().into_owned(),
                    options: model::ImportOptions::default(),
                })
                .unwrap();
            controller
                .dispatch(controller::action::Action::ConfirmImport)
                .unwrap();
            if path == &original_csv {
                let series_id = controller.model.series[0].id;
                controller
                    .dispatch(controller::action::Action::RenameSeries {
                        series_id,
                        name: "Pressure over time".to_owned(),
                    })
                    .unwrap();
                controller
                    .dispatch(controller::action::Action::SetSeriesLineStyle {
                        series_id,
                        line_style: model::LineStyle::Dashed,
                    })
                    .unwrap();
                controller
                    .dispatch(controller::action::Action::SetSeriesMarker {
                        series_id,
                        marker: Some(model::MarkerShape::Square),
                        size: 8.0,
                    })
                    .unwrap();
                controller
                    .dispatch(controller::action::Action::SetLegendPosition(
                        model::LegendPosition::OutsideRight,
                    ))
                    .unwrap();
            }
        }
        let series_id = controller.model.series[0].id;
        assert_eq!(controller.model.series[0].x_column, "time");
        assert_eq!(controller.model.series[0].y_column, "signal");
        assert_eq!(
            controller.points_for_series(series_id).unwrap(),
            vec![(0.0, 5.0), (1.0, 7.0), (2.0, 9.0)]
        );

        let dimensions = model::PublicationDimensions {
            width: 90.0,
            height: 60.0,
            unit: model::PhysicalUnit::Millimeters,
            dpi: 300,
        };
        controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions,
            })
            .unwrap();
        let expected_model = controller.model.clone();
        let mut reopened = PlotController::new();
        reopened
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert!(reopened.model == expected_model);
        assert_eq!(reopened.project_publication_dimensions(), Some(dimensions));
        assert_eq!(
            reopened.data_summary().unwrap().source,
            reordered_csv.file_name().unwrap().to_string_lossy()
        );

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                EditorFixture {
                    controller: reopened,
                    view: PlotEditorView::new(),
                },
            );
        harness.run();
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness.get_by_label_contains("Pressure over time | time -> signal");
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 1063,
                    height: 709,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&svg_path).unwrap();
        assert!(svg.contains("Pressure over time"));
        assert!(svg.contains("viewBox=\"0 0 1063 709\""));
        std::fs::remove_file(original_csv).unwrap();
        std::fs::remove_file(reordered_csv).unwrap();
        std::fs::remove_file(project_path).unwrap();
        std::fs::remove_file(svg_path).unwrap();
    }

    #[test]
    fn malformed_headerless_import_preview_cannot_replace_the_active_table() {
        let mut fixture = demo_fixture();
        let original_summary = fixture.controller.data_summary().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "kithe_plot_invalid_import_ui_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("invalid.csv");
        std::fs::write(&csv_path, "0,NaN\n1,2\n").unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::PreviewImport {
                path: csv_path.to_string_lossy().into_owned(),
                options: model::ImportOptions::default(),
            })
            .unwrap();

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.get_by_label("Header row").click();
        harness.run_ok();
        harness.get_by_label("No header row").click();
        harness.run_ok();
        harness.get_by_label_contains("Non-finite");
        harness.get_by_label("Import data").click();
        harness.run_ok();

        assert_eq!(
            harness.state().controller.data_summary().unwrap(),
            original_summary
        );
        assert!(harness.state().controller.import_preview().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn chart_title_text_is_committed_on_enter_and_preserves_trailing_spaces() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                {
                    let mut fixture = demo_fixture();
                    fixture.controller.model.layout.title.clear();
                    fixture
                },
            );

        harness.get_by_label("Chart title & typography").click();
        harness.run_ok();
        let title = harness.get_by_label("Chart title");
        title.click();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        title.type_text("Result ");
        harness.run();
        assert_eq!(harness.state().controller.model.layout.title, "");
        harness.key_press(egui::Key::Enter);
        harness.run_ok();
        assert_eq!(harness.state().controller.model.layout.title, "Result ");
    }

    #[test]
    fn axis_title_position_control_enables_manual_anchor() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );

        harness.get_by_label("X Axis").click();
        harness.run_ok();
        harness.get_by_label("Manual title position").click();
        harness.run_ok();
        assert!(
            harness
                .state()
                .controller
                .model
                .axes
                .x
                .title_anchor
                .is_some()
        );
    }

    #[test]
    fn plot_preview_snapshot() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );
        harness.snapshot("plot_preview");
    }

    #[test]
    fn annotation_is_visible_in_preview_export_and_undo_redo() {
        let mut fixture = demo_fixture();
        let annotation = model::PlotAnnotation {
            id: 1,
            text: "peak".to_owned(),
            x: 1.5,
            y: 0.85,
            font_size: 18,
            color: model::Color {
                r: 20,
                g: 80,
                b: 160,
                a: 255,
            },
        };
        fixture
            .controller
            .dispatch(controller::action::Action::SetAnnotations(vec![annotation]))
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.snapshot("plot_preview_annotation");

        let path = std::env::temp_dir().join(format!(
            "kitheplot_annotation_{}.svg",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 640,
                    height: 480,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).unwrap();
        assert!(svg.contains("peak"));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(harness.state().controller.model.annotations.is_empty());
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(harness.state().controller.model.annotations.len(), 1);
        let project_path = std::env::temp_dir().join(format!(
            "kitheplot_annotation_{}.kplot",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(harness.state().controller.model.annotations.len(), 1);
        std::fs::remove_file(project_path).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn annotation_controls_add_and_remove_annotations_undoably() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );
        harness.get_by_label("Annotations").click();
        harness.run_ok();
        harness.get_by_label("Add annotation").click();
        harness.run_ok();
        assert_eq!(harness.state().controller.model.annotations.len(), 1);
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(harness.state().controller.model.annotations.is_empty());
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(harness.state().controller.model.annotations.len(), 1);
        harness.get_by_label("Remove").click();
        harness.run_ok();
        assert!(harness.state().controller.model.annotations.is_empty());
    }

    #[test]
    fn linked_plot_panels_render_export_and_persist_membership() {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .load_from_data_source(&PanelOutlierData)
            .expect("panel data should load");
        let first_series = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::AddSeries {
                name: "Second signal".to_owned(),
                x_column: "time".to_owned(),
                y_column: "signal".to_owned(),
            })
            .unwrap();
        let second_series = fixture.controller.model.series[1].id;
        fixture
            .controller
            .dispatch(controller::action::Action::AddSeries {
                name: "Unassigned outlier".to_owned(),
                x_column: "time".to_owned(),
                y_column: "outlier".to_owned(),
            })
            .unwrap();
        let outlier_series = fixture.controller.model.series[2].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetPanels(vec![
                model::PlotPanel {
                    id: 1,
                    title: "Panel A".to_owned(),
                    series_ids: vec![first_series],
                },
                model::PlotPanel {
                    id: 2,
                    title: "Panel B".to_owned(),
                    series_ids: vec![second_series],
                },
            ]))
            .unwrap();

        let (y_min, y_max) = fixture
            .controller
            .suggested_axis_range(model::AxisKind::Y)
            .expect("assigned panel data should determine the range");
        assert!(y_min > -2.0 && y_max < 2.0, "{y_min}..{y_max}");
        assert!(
            !fixture
                .controller
                .model
                .panels
                .iter()
                .any(|panel| { panel.series_ids.contains(&outlier_series) })
        );

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.snapshot("plot_preview_two_panels");
        assert!(harness.state().view.last_plot_rect().unwrap().is_positive());
        let (x_min, x_max) = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::X)
            .unwrap();
        let (y_min, y_max) = harness
            .state()
            .controller
            .suggested_axis_range(model::AxisKind::Y)
            .unwrap();
        let x: f64 = 1.5;
        let y = x.sin();
        for (panel_index, expected_series) in [(0, first_series), (1, second_series)] {
            let rect = harness.state().view.last_panel_data_rects()[panel_index];
            let point = egui::pos2(
                rect.left() + ((x - x_min) / (x_max - x_min)) as f32 * rect.width(),
                rect.bottom() - ((y - y_min) / (y_max - y_min)) as f32 * rect.height(),
            );
            harness.drag_at(point);
            harness.run_ok();
            harness.drop_at(point);
            harness.run_ok();
            assert_eq!(
                harness.state().view.selected_series(),
                Some(expected_series)
            );
        }
        let project_path = std::env::temp_dir().join(format!(
            "kitheplot_panels_{}.kplot",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SaveProject {
                path: project_path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::OpenProject {
                path: project_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(harness.state().controller.model.panels.len(), 2);
        assert_eq!(
            harness.state().controller.model.panels[1].series_ids,
            vec![second_series]
        );

        let svg_path = project_path.with_extension("svg");
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 900,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&svg_path).unwrap();
        assert!(svg.contains("Panel A") && svg.contains("Panel B"));
        assert!(!svg.contains("Unassigned outlier"));
        std::fs::remove_file(project_path).unwrap();
        std::fs::remove_file(svg_path).unwrap();
    }

    #[test]
    fn plot_panel_controls_create_linked_panel_undoably() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );
        harness.get_by_label("Plot panels").click();
        harness.run_ok();
        harness.get_by_label("Add plot panel").click();
        harness.run_ok();
        let panels = &harness.state().controller.model.panels;
        assert_eq!(panels.len(), 1);
        assert_eq!(
            panels[0].series_ids,
            vec![harness.state().controller.model.series[0].id]
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(harness.state().controller.model.panels.is_empty());
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(harness.state().controller.model.panels.len(), 1);
    }

    #[test]
    fn portable_style_applies_appearance_without_replacing_data_semantics() {
        let mut source = demo_fixture();
        source.controller.model.layout.margin = 24;
        source.controller.model.layout.title_font_size = 38;
        source.controller.model.legend.font_size = 21;
        source.controller.model.series[0].style.color = model::Color {
            r: 12,
            g: 90,
            b: 170,
            a: 200,
        };
        source.controller.model.series[0].style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 8.0,
        });
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let style_path = std::env::temp_dir().join(format!("kitheplot_style_{suffix}.kstyle"));
        source
            .controller
            .dispatch(controller::action::Action::SavePlotStyle {
                path: style_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&style_path).unwrap()).unwrap();
        assert!(document.get("data").is_none());
        assert!(document.get("series").is_some());

        let mut target = demo_fixture();
        target.controller.model.layout.title = "Target experiment".to_owned();
        target.controller.model.axes.x.label = "Elapsed time".to_owned();
        target.controller.model.axes.x.range = model::RangePolicy::Manual { min: 1.0, max: 4.0 };
        target.controller.model.series[0].x_column = "time".to_owned();
        target.controller.model.series[0].y_column = "signal".to_owned();
        target
            .controller
            .dispatch(controller::action::Action::AddSeries {
                name: "Extra".to_owned(),
                x_column: "time".to_owned(),
                y_column: "signal".to_owned(),
            })
            .unwrap();
        target.controller.model.series[1].style.color = model::Color {
            r: 10,
            g: 200,
            b: 30,
            a: 255,
        };
        let extra_color = target.controller.model.series[1].style.color;
        target
            .controller
            .dispatch(controller::action::Action::ApplyPlotStyle {
                path: style_path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(target.controller.model.layout.title, "Target experiment");
        assert_eq!(target.controller.model.axes.x.label, "Elapsed time");
        assert_eq!(
            target.controller.model.axes.x.range,
            model::RangePolicy::Manual { min: 1.0, max: 4.0 }
        );
        assert_eq!(target.controller.model.series[0].style.color.r, 12);
        assert_eq!(
            target.controller.model.series[0]
                .style
                .marker
                .as_ref()
                .unwrap()
                .shape,
            model::MarkerShape::Square
        );
        assert_eq!(target.controller.model.series[1].style.color, extra_color);
        assert_eq!(target.controller.model.series[0].x_column, "time");

        target
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_ne!(target.controller.model.series[0].style.color.r, 12);
        assert_eq!(target.controller.model.layout.title, "Target experiment");
        std::fs::remove_file(style_path).unwrap();
    }

    #[test]
    fn portable_style_embeds_and_restores_its_custom_font_asset() {
        let context = egui::Context::default();
        let asset_id = register_chart_font(
            &context,
            "Portable Style Sans".to_owned(),
            include_bytes!("../assets/fonts/NotoSans.ttf").to_vec(),
        )
        .unwrap();
        let mut source = demo_fixture();
        source.controller.model.layout.font_family = model::PlotFontFamily::CustomAsset {
            family: "Portable Style Sans".to_owned(),
            asset_id: asset_id.clone(),
        };
        let path = std::env::temp_dir().join(format!(
            "kitheplot_font_style_{}.kstyle",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        source
            .controller
            .dispatch(controller::action::Action::SavePlotStyle {
                path: path.to_string_lossy().into_owned(),
            })
            .unwrap();
        let mut target = demo_fixture();
        target
            .controller
            .dispatch(controller::action::Action::ApplyPlotStyle {
                path: path.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert_eq!(
            target.controller.model.layout.font_family,
            model::PlotFontFamily::CustomAsset {
                family: "Portable Style Sans".to_owned(),
                asset_id,
            }
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn plot_preview_typography_and_markers_snapshot() {
        let mut fixture = demo_fixture();
        fixture.controller.model.layout.title = "Измерение β — тест".to_owned();
        fixture.controller.model.axes.x.label = "Время, мс".to_owned();
        fixture.controller.model.axes.y.label = "Амплитуда, мВ".to_owned();
        fixture.controller.model.axes.x.label_font_size = 26;
        fixture.controller.model.axes.y.label_font_size = 26;
        fixture.controller.model.axes.x.axis_title_font_size = 20;
        fixture.controller.model.axes.y.axis_title_font_size = 20;
        fixture.controller.model.series[0].name = "Эксперимент α".to_owned();
        fixture.controller.model.series[0].style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 9.0,
        });
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot("plot_preview_typography_and_markers");
    }

    #[test]
    fn plot_preview_step_interpolation_snapshot() {
        let mut fixture = demo_fixture();
        fixture.controller.model.series[0].style.line_interpolation =
            model::LineInterpolation::StepAfter;
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot("plot_preview_step_interpolation");
    }

    #[test]
    fn area_series_uses_saved_baseline_in_preview_and_export() {
        let mut fixture = demo_fixture();
        let series_id = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesAreaBaseline {
                series_id,
                baseline: Some(-2.0),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(
            fixture.controller.model.series[0]
                .style
                .area_baseline
                .is_none()
        );
        fixture
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(
            fixture.controller.model.series[0].style.area_baseline,
            Some(-2.0)
        );

        let path =
            std::env::temp_dir().join(format!("kithe_plot_area_series_{}.svg", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("area SVG should be readable");
        std::fs::remove_file(path).expect("temporary SVG should be removed");
        assert!(svg.contains("<svg"));
        assert!(
            svg.matches("<polygon").count() >= 40,
            "area should export as many local filled triangles, not one concave polygon"
        );

        let png_path =
            std::env::temp_dir().join(format!("kithe_plot_area_series_{}.png", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: png_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Png,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let png = std::fs::read(&png_path).expect("area PNG should be readable");
        std::fs::remove_file(png_path).expect("temporary PNG should be removed");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 800);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 600);

        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot("plot_preview_area_series");
    }

    fn snapshot_outside_legend(position: model::LegendPosition, name: &str) {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .dispatch(controller::action::Action::AddSeries {
                name: "Second Series".to_owned(),
                x_column: "time".to_owned(),
                y_column: "signal".to_owned(),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendPosition(position))
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot(name);
    }

    #[test]
    fn plot_preview_outside_left_legend_snapshot() {
        snapshot_outside_legend(
            model::LegendPosition::OutsideLeft,
            "plot_preview_outside_left",
        );
    }

    #[test]
    fn plot_preview_outside_right_legend_snapshot() {
        snapshot_outside_legend(
            model::LegendPosition::OutsideRight,
            "plot_preview_outside_right",
        );
    }

    #[test]
    fn plot_preview_outside_top_legend_snapshot() {
        snapshot_outside_legend(
            model::LegendPosition::OutsideTop,
            "plot_preview_outside_top",
        );
    }

    #[test]
    fn plot_preview_outside_bottom_legend_snapshot() {
        snapshot_outside_legend(
            model::LegendPosition::OutsideBottom,
            "plot_preview_outside_bottom",
        );
    }

    #[test]
    fn manual_range_clipping_matches_preview_and_svg_export() {
        let mut fixture = error_bar_fixture();
        let series = &mut fixture.controller.model.series[0];
        series.style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 12.0,
        });
        series.style.area_fill_visible = true;
        series.style.area_baseline = Some(10.0);
        fixture.controller.model.axes.x.range = model::RangePolicy::Manual {
            min: 11.0,
            max: 19.0,
        };
        fixture.controller.model.axes.y.range = model::RangePolicy::Manual {
            min: 11.0,
            max: 19.0,
        };
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        harness.snapshot("plot_preview_manual_range_clipping");

        let path = std::env::temp_dir().join(format!(
            "kithe_plot_clipped_export_{}.svg",
            std::process::id()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(svg.contains("<polygon"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn dragging_legend_updates_its_saved_anchor() {
        let fixture = demo_fixture();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        let starting_anchor = harness.state().controller.model.legend.anchor;
        let starting_position = harness.state().controller.model.legend.position;
        let legend_rect = harness.state().view.last_legend_rect().unwrap();
        let start = legend_rect.center();
        let target = start + egui::vec2(40.0, 0.0);
        harness.drag_at(start);
        harness.run_ok();
        harness.hover_at(target);
        harness.run_ok();
        let moving_legend = harness.state().view.last_legend_rect().unwrap();
        assert!(moving_legend.center().distance(target) < 2.0);
        harness.drop_at(target);
        harness.run_ok();
        let released_legend = harness.state().view.last_legend_rect().unwrap();
        assert!(released_legend.center().distance(target) < 2.0);
        assert_ne!(
            harness.state().controller.model.legend.anchor,
            starting_anchor
        );
        assert_eq!(
            harness.state().controller.model.legend.position,
            model::LegendPosition::Custom
        );
        let path = std::env::temp_dir().join(format!(
            "kithe_plot_dragged_legend_{}.kplot",
            std::process::id()
        ));
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SaveProject {
                path: path.to_string_lossy().into_owned(),
                dimensions: model::PublicationDimensions::default(),
            })
            .unwrap();
        let mut reopened = PlotController::new();
        reopened
            .dispatch(controller::action::Action::OpenProject {
                path: path.to_string_lossy().into_owned(),
            })
            .unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            reopened.model.legend.position,
            model::LegendPosition::Custom
        );
        assert_eq!(
            reopened.model.legend.anchor,
            harness.state().controller.model.legend.anchor
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.legend.anchor,
            starting_anchor
        );
        assert_eq!(
            harness.state().controller.model.legend.position,
            starting_position
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        harness.run_ok();
        let redone_legend = harness.state().view.last_legend_rect().unwrap();
        assert!(redone_legend.center().distance(target) < 2.0);
    }

    #[test]
    fn legend_drag_works_from_each_position_preset_without_reselection() {
        for position in [
            model::LegendPosition::OutsideTop,
            model::LegendPosition::OutsideBottom,
            model::LegendPosition::OutsideLeft,
            model::LegendPosition::OutsideRight,
        ] {
            let mut fixture = demo_fixture();
            fixture
                .controller
                .dispatch(controller::action::Action::SetLegendPosition(position))
                .unwrap();
            let mut harness = Harness::builder()
                .with_accessibility_check(false)
                .build_ui_state(
                    |ui, fixture| {
                        ui.ctx().set_visuals(egui::Visuals::light());
                        let actions = fixture.view.draw(ui, &fixture.controller);
                        for action in actions {
                            let _ = fixture.controller.dispatch(action);
                        }
                    },
                    fixture,
                );
            harness.run();
            let initial = harness.state().view.last_legend_rect().unwrap();
            let start = initial.center();
            let delta = match position {
                model::LegendPosition::OutsideTop => egui::vec2(24.0, 12.0),
                model::LegendPosition::OutsideBottom => egui::vec2(24.0, -12.0),
                model::LegendPosition::OutsideLeft => egui::vec2(24.0, 12.0),
                model::LegendPosition::OutsideRight => egui::vec2(-24.0, 12.0),
                _ => unreachable!(),
            };
            let target = start + delta;
            harness.drag_at(start);
            harness.run_ok();
            for fraction in [0.3, 0.65, 1.0] {
                let point = start + (target - start) * fraction;
                harness.hover_at(point);
                harness.run_ok();
                let moved = harness.state().view.last_legend_rect().unwrap();
                assert!(
                    moved.center().distance(point) < 2.0,
                    "legend preset {position:?} did not follow the pointer at {fraction}"
                );
            }
            harness.drop_at(target);
            harness.run_ok();
            assert!(
                harness
                    .state()
                    .view
                    .last_legend_rect()
                    .unwrap()
                    .center()
                    .distance(target)
                    < 2.0,
                "legend preset {position:?} did not stay at its released position"
            );
            assert_eq!(
                harness.state().controller.model.legend.position,
                model::LegendPosition::Custom
            );
            harness
                .state_mut()
                .controller
                .dispatch(controller::action::Action::Undo)
                .unwrap();
            harness.run_ok();
            assert_eq!(harness.state().controller.model.legend.position, position);
            harness
                .state_mut()
                .controller
                .dispatch(controller::action::Action::Redo)
                .unwrap();
            harness.run_ok();
            assert!(
                harness
                    .state()
                    .view
                    .last_legend_rect()
                    .unwrap()
                    .center()
                    .distance(target)
                    < 2.0
            );
        }
    }

    #[test]
    fn changing_legend_border_width_preserves_rgba_colors() {
        let mut fixture = demo_fixture();
        fixture.controller.model.legend.background_color = model::Color {
            r: 230,
            g: 170,
            b: 90,
            a: 160,
        };
        fixture.controller.model.legend.border_color = model::Color {
            r: 80,
            g: 140,
            b: 210,
            a: 120,
        };
        let original_background = fixture.controller.model.legend.background_color;
        let original_border = fixture.controller.model.legend.border_color;
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );

        harness.get_by_label("Legend").click();
        harness.run_ok();
        let border_width =
            harness.get_by_role_and_label(egui::accesskit::Role::Slider, "Border width");
        border_width.scroll_to_me();
        harness.run_ok();
        harness
            .get_by_role_and_label(egui::accesskit::Role::Slider, "Border width")
            .set_value_accesskit(8.0);
        harness.run_ok();

        let legend = &harness.state().controller.model.legend;
        let channels = |color: model::Color| (color.r, color.g, color.b, color.a);
        assert_eq!(
            channels(legend.background_color),
            channels(original_background)
        );
        assert_eq!(channels(legend.border_color), channels(original_border));
        assert_eq!(legend.border_width, 8.0);
    }

    #[test]
    fn dragging_legend_border_width_groups_updates_into_one_undo() {
        let fixture = demo_fixture();
        let initial_width = fixture.controller.model.legend.border_width;
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Legend").click();
        harness.run_ok();
        let slider = harness.get_by_role_and_label(egui::accesskit::Role::Slider, "Border width");
        slider.scroll_to_me();
        harness.run_ok();
        let slider_rect = harness
            .get_by_role_and_label(egui::accesskit::Role::Slider, "Border width")
            .rect();
        let start = slider_rect.center();
        let target = egui::pos2(slider_rect.right() - 3.0, slider_rect.center().y);
        harness.drag_at(start);
        harness.run_ok();
        harness.hover_at(target);
        harness.run_ok();
        harness.hover_at(egui::pos2(target.x - 12.0, target.y));
        harness.run_ok();
        let during_drag_history_len = harness.state().controller.undo_stack.len();
        harness.drop_at(target);
        harness.run_ok();

        assert!(harness.state().controller.model.legend.border_width > initial_width);
        assert_eq!(harness.state().controller.undo_stack.len(), 1);
        assert_eq!(during_drag_history_len, 0);
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(
            harness.state().controller.model.legend.border_width,
            initial_width
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert!(harness.state().controller.model.legend.border_width > initial_width);
    }

    #[test]
    fn editing_legend_color_from_the_picker_is_undoable() {
        let fixture = demo_fixture();
        let initial_color = fixture.controller.model.legend.background_color;
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Legend").click();
        harness.run_ok();
        harness
            .get_by_role_and_label(egui::accesskit::Role::ColorWell, "Legend background")
            .click();
        harness.run_ok();
        harness
            .get_by_role_and_label(egui::accesskit::Role::SpinButton, "R")
            .set_value_accesskit(120.0);
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.legend.background_color.r,
            120
        );
        assert_eq!(harness.state().controller.undo_stack.len(), 1);
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(
            harness.state().controller.model.legend.background_color,
            initial_color
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(
            harness.state().controller.model.legend.background_color.r,
            120
        );
    }

    #[test]
    fn dragging_grid_width_slider_is_one_undo_operation() {
        let fixture = demo_fixture();
        let initial_width = fixture.controller.model.axes.x.ticks.major_grid_width;
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("X Axis").click();
        harness.run_ok();
        let slider = harness.get_by_role_and_label(egui::accesskit::Role::Slider, "Major width");
        slider.scroll_to_me();
        harness.run_ok();
        let slider_rect = harness
            .get_by_role_and_label(egui::accesskit::Role::Slider, "Major width")
            .rect();
        let start = slider_rect.center();
        let target = egui::pos2(slider_rect.right() - 3.0, slider_rect.center().y);
        harness.drag_at(start);
        harness.run_ok();
        harness.hover_at(target);
        harness.run_ok();
        harness.hover_at(egui::pos2(target.x - 12.0, target.y));
        harness.run_ok();
        let history_during_drag = harness.state().controller.undo_stack.len();
        harness.drop_at(target);
        harness.run_ok();

        assert!(
            harness
                .state()
                .controller
                .model
                .axes
                .x
                .ticks
                .major_grid_width
                > initial_width
        );
        assert_eq!(history_during_drag, 0);
        assert_eq!(harness.state().controller.undo_stack.len(), 1);
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(
            harness
                .state()
                .controller
                .model
                .axes
                .x
                .ticks
                .major_grid_width,
            initial_width
        );
    }

    #[test]
    fn svg_preserves_configured_alpha_for_title_series_and_grid() {
        let mut fixture = demo_fixture();
        fixture.controller.model.layout.title_font_color.a = 128;
        fixture.controller.model.series[0].style.color.a = 128;
        fixture.controller.model.axes.x.ticks.major_grid_color.a = 128;
        fixture.controller.model.axes.y.ticks.major_grid_color.a = 128;
        let path =
            std::env::temp_dir().join(format!("kithe_plot_alpha_{}.svg", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("alpha SVG should be readable");
        std::fs::remove_file(path).expect("temporary alpha SVG should be removed");
        assert!(
            svg.contains("opacity=\"0.5019"),
            "expected configured alpha in SVG opacity attributes"
        );
    }

    #[test]
    fn dragging_axis_title_updates_its_saved_anchor() {
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                demo_fixture(),
            );
        harness.run();
        let plot_rect = harness.state().view.last_plot_rect().unwrap();
        let hitboxes = harness.state().view.last_axis_title_hitboxes().unwrap();
        let start = hitboxes
            .iter()
            .find_map(|(axis, rect)| (*axis == model::AxisKind::X).then_some(rect.center()))
            .unwrap();
        let delta = egui::vec2(40.0, 0.0);
        let target = start + delta;
        harness.drag_at(start);
        harness.run_ok();
        harness.hover_at(target);
        harness.run_ok();
        let moving_hitboxes = harness.state().view.last_axis_title_hitboxes().unwrap();
        let moving_center = moving_hitboxes
            .iter()
            .find_map(|(axis, rect)| (*axis == model::AxisKind::X).then_some(rect.center()))
            .unwrap();
        assert!(moving_center.distance(target) < 2.0);
        harness.drop_at(target);
        harness.run_ok();
        let anchor = harness
            .state()
            .controller
            .model
            .axes
            .x
            .title_anchor
            .expect("dragging the visible X title should save its anchor");
        let actual = egui::pos2(
            plot_rect.left() + anchor[0] * plot_rect.width(),
            plot_rect.top() + anchor[1] * plot_rect.height(),
        );
        assert!(
            actual.distance(target) < 2.0,
            "expected {target:?}, got {actual:?}"
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.axes.x.title_anchor,
            None,
            "one Undo should cancel the whole axis-title drag"
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.axes.x.title_anchor,
            Some(anchor),
            "one Redo should restore the final axis-title position"
        );
    }

    #[test]
    fn series_line_control_toggles_line_without_removing_markers() {
        let mut fixture = demo_fixture();
        let series = &mut fixture.controller.model.series[0];
        series.style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 5.0,
        });
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness
            .get_by_label_contains("Series 1 | time -> signal")
            .click();
        harness.run_ok();
        harness.get_by_label("Connect points").scroll_to_me();
        harness.run_ok();
        harness.get_by_label("Connect points").click();
        harness.run_ok();
        let series = &harness.state().controller.model.series[0];
        assert!(!series.style.line_visible);
        assert_eq!(
            series.style.marker.as_ref().unwrap().shape,
            model::MarkerShape::Square
        );
    }

    #[test]
    fn disabling_marker_keeps_its_shape_and_size_for_reenable() {
        let mut fixture = demo_fixture();
        fixture.controller.model.series[0].style.marker = Some(model::MarkerStyle {
            shape: model::MarkerShape::Square,
            size: 12.0,
        });
        fixture.controller.model.series[0].style.marker_preset = fixture.controller.model.series[0]
            .style
            .marker
            .clone()
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness
            .get_by_label_contains("Series 1 | time -> signal")
            .click();
        harness.run_ok();
        harness.get_by_label("Marker").scroll_to_me();
        harness.run_ok();
        harness.get_by_label("Marker").click();
        harness.run_ok();
        assert!(
            harness.state().controller.model.series[0]
                .style
                .marker
                .is_none()
        );
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .marker_preset,
            model::MarkerStyle {
                shape: model::MarkerShape::Square,
                size: 12.0,
            }
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .marker
                .as_ref(),
            Some(&model::MarkerStyle {
                shape: model::MarkerShape::Square,
                size: 12.0,
            })
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        harness.run_ok();
        harness.get_by_label("Marker").click();
        harness.run_ok();
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .marker
                .as_ref(),
            Some(&model::MarkerStyle {
                shape: model::MarkerShape::Square,
                size: 12.0,
            })
        );
    }

    #[test]
    fn disabling_area_fill_keeps_baseline_and_log_axis_uses_valid_baseline() {
        let mut fixture = demo_fixture();
        let series_id = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesAreaBaseline {
                series_id,
                baseline: Some(-0.5),
            })
            .unwrap();
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    ui.ctx().set_visuals(egui::Visuals::light());
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.get_by_label("Series").click();
        harness.run_ok();
        harness
            .get_by_label_contains("Series 1 | time -> signal")
            .click();
        harness.run_ok();
        harness.get_by_label("Area fill").scroll_to_me();
        harness.run_ok();
        harness.get_by_label("Area fill").click();
        harness.run_ok();
        assert!(
            !harness.state().controller.model.series[0]
                .style
                .area_fill_visible
        );
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .area_baseline,
            Some(-0.5)
        );
        harness.get_by_label("Area fill").click();
        harness.run_ok();
        assert!(
            harness.state().controller.model.series[0]
                .style
                .area_fill_visible
        );
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .area_baseline,
            Some(-0.5)
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        harness.run_ok();
        assert!(
            !harness.state().controller.model.series[0]
                .style
                .area_fill_visible
        );
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        harness.run_ok();

        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SetAxisScale {
                axis: model::AxisKind::Y,
                scale: model::ScaleType::Log10Axis,
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SetSeriesAreaVisibility {
                series_id,
                visible: false,
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SetSeriesAreaBaseline {
                series_id,
                baseline: None,
            })
            .unwrap();
        harness
            .state_mut()
            .controller
            .dispatch(controller::action::Action::SetSeriesAreaVisibility {
                series_id,
                visible: true,
            })
            .unwrap();
        let style = &harness.state().controller.model.series[0].style;
        assert!(style.area_fill_visible);
        assert!(style.area_baseline.unwrap() > 0.0);
        let baseline = style.area_baseline;
        assert!(
            harness
                .state_mut()
                .controller
                .dispatch(controller::action::Action::SetSeriesAreaBaseline {
                    series_id,
                    baseline: Some(0.0),
                })
                .is_err()
        );
        assert_eq!(
            harness.state().controller.model.series[0]
                .style
                .area_baseline,
            baseline
        );
    }

    #[test]
    fn svg_export_contains_titles_and_custom_legend() {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .dispatch(controller::action::Action::AddSeries {
                name: "Second Series".to_owned(),
                x_column: "time".to_owned(),
                y_column: "signal".to_owned(),
            })
            .unwrap();
        fixture.controller.model.legend.columns = 2;
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTickNumberFormat {
                axis: model::AxisKind::Y,
                format: model::TickNumberFormat::Scientific { decimals: 2 },
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendAnchor([0.3, 0.25]))
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTitleAnchor {
                axis: model::AxisKind::X,
                anchor: Some([0.4, 0.82]),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendPosition(
                model::LegendPosition::OutsideRight,
            ))
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("kithe_plot_wysiwyg_{}.svg", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("SVG export should be readable");
        std::fs::remove_file(&path).expect("temporary SVG should be removed");
        assert!(svg.contains("Legend"));
        assert!(svg.contains("@font-face"));
        assert!(svg.contains("data:font/ttf;base64,"));
        assert!(svg.contains("Series 1"));
        assert!(svg.contains("Second Series"));
        assert!(svg.contains("X"));
        assert!(svg.contains("1.00e0"));

        let png_path = path.with_extension("png");
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: png_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Png,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let png = std::fs::read(&png_path).expect("PNG export should be readable");
        std::fs::remove_file(png_path).expect("temporary PNG should be removed");
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn registered_host_font_is_available_to_svg_export() {
        let family = format!("Host Cyrillic {}", std::process::id());
        let font_bytes = include_bytes!("../assets/fonts/NotoSans.ttf").to_vec();
        let asset_id = register_chart_font(
            &egui::Context::default(),
            family.clone(),
            font_bytes.clone(),
        )
        .expect("host font should register");
        let mut export_fixture = demo_fixture();
        export_fixture.controller.model.layout.font_family = model::PlotFontFamily::CustomAsset {
            family: family.clone(),
            asset_id: asset_id.clone(),
        };
        let mut fixture = demo_fixture();
        fixture.controller.model.layout.font_family = model::PlotFontFamily::CustomAsset {
            family: family.clone(),
            asset_id: asset_id.clone(),
        };
        let mut harness = Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, fixture| {
                    assert_eq!(
                        register_chart_font(ui.ctx(), family.clone(), font_bytes.clone(),)
                            .expect("a valid host font should register"),
                        asset_id
                    );
                    let actions = fixture.view.draw(ui, &fixture.controller);
                    for action in actions {
                        let _ = fixture.controller.dispatch(action);
                    }
                },
                fixture,
            );
        harness.run();
        let mut fixture = export_fixture;
        let path =
            std::env::temp_dir().join(format!("kithe_plot_custom_font_{}.svg", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 800,
                    height: 600,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("custom-font SVG should be readable");
        std::fs::remove_file(path).expect("temporary SVG should be removed");
        assert!(svg.contains("Plot"));
        assert!(svg.contains(&format!("font-family:'KiThePlot Custom {asset_id}'")));
        assert!(svg.contains("data:font/ttf;base64,"));
    }

    #[test]
    fn publication_export_records_physical_size_and_png_resolution() {
        let mut fixture = demo_fixture();
        let dimensions = model::PublicationDimensions {
            width: 25.4,
            height: 12.7,
            unit: model::PhysicalUnit::Millimeters,
            dpi: 100,
        };
        let pixel_size = dimensions.pixel_size().expect("valid dimensions");
        assert_eq!((pixel_size.width, pixel_size.height), (100, 50));
        let inch_dimensions = model::PublicationDimensions {
            width: 1.0,
            height: 0.5,
            unit: model::PhysicalUnit::Inches,
            ..dimensions
        };
        let inch_pixels = inch_dimensions
            .pixel_size()
            .expect("inch dimensions should be valid");
        assert_eq!((inch_pixels.width, inch_pixels.height), (100, 50));

        let svg_path =
            std::env::temp_dir().join(format!("kithe_plot_publication_{}.svg", std::process::id()));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestPublicationExport {
                path: svg_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                dimensions,
            })
            .expect("physical SVG export should succeed");
        let svg = std::fs::read_to_string(&svg_path).expect("SVG should be readable");
        std::fs::remove_file(&svg_path).expect("SVG fixture should be removed");
        assert!(svg.contains("width=\"25.4000mm\""));
        assert!(svg.contains("height=\"12.7000mm\""));
        assert!(svg.contains("viewBox=\"0 0 100 50\""));

        let png_path = svg_path.with_extension("png");
        fixture
            .controller
            .dispatch(controller::action::Action::RequestPublicationExport {
                path: png_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Png,
                dimensions,
            })
            .expect("physical PNG export should succeed");
        let png = std::fs::read(&png_path).expect("PNG should be readable");
        std::fs::remove_file(&png_path).expect("PNG fixture should be removed");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 100);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 50);

        let mut offset = 8;
        let mut found_resolution = false;
        while offset + 12 <= png.len() {
            let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let kind = &png[offset + 4..offset + 8];
            if kind == b"pHYs" {
                let payload = &png[offset + 8..offset + 8 + length];
                let expected_ppm = (100.0_f64 / 0.0254).round() as u32;
                assert_eq!(
                    u32::from_be_bytes(payload[0..4].try_into().unwrap()),
                    expected_ppm
                );
                assert_eq!(
                    u32::from_be_bytes(payload[4..8].try_into().unwrap()),
                    expected_ppm
                );
                assert_eq!(payload[8], 1);
                let checksum_start = offset + 8 + length;
                let checksum =
                    u32::from_be_bytes(png[checksum_start..checksum_start + 4].try_into().unwrap());
                assert_eq!(checksum, crc32fast::hash(&png[offset + 4..checksum_start]));
                found_resolution = true;
                break;
            }
            offset += length + 12;
        }
        assert!(found_resolution, "PNG should include its DPI metadata");
    }

    #[test]
    fn svg_export_writes_long_negative_tick_labels() {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisRange {
                axis: model::AxisKind::Y,
                range: model::RangePolicy::Manual {
                    min: -2_000_000.0,
                    max: -1_000_000.0,
                },
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisMajorTickStep {
                axis: model::AxisKind::Y,
                step: Some(500_000.0),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTickNumberFormat {
                axis: model::AxisKind::Y,
                format: model::TickNumberFormat::Fixed { decimals: 0 },
            })
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "kithe_plot_negative_ticks_{}.svg",
            std::process::id()
        ));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 1200,
                    height: 800,
                },
            })
            .unwrap();
        let svg = std::fs::read_to_string(&path).expect("SVG export should be readable");
        std::fs::remove_file(path).expect("temporary SVG should be removed");
        assert!(svg.contains("width=\"1200\""));
        assert!(svg.contains("height=\"800\""));
        assert!(svg.contains("-2000000"));
        assert!(svg.contains("-1500000"));
        assert!(svg.contains("-1000000"));

        let mut large_output_font_sizes: Vec<&str> = svg
            .split("font-size=\"")
            .skip(1)
            .filter_map(|value| value.split('\"').next())
            .collect();
        large_output_font_sizes.sort_unstable();
        large_output_font_sizes.dedup();
        let small_path = std::env::temp_dir().join(format!(
            "kithe_plot_negative_ticks_small_{}.svg",
            std::process::id()
        ));
        fixture
            .controller
            .dispatch(controller::action::Action::RequestSaveAs {
                path: small_path.to_string_lossy().into_owned(),
                format: model::ImageFormat::Svg,
                size: model::ImageSize {
                    width: 600,
                    height: 400,
                },
            })
            .unwrap();
        let small_svg =
            std::fs::read_to_string(&small_path).expect("smaller SVG export should be readable");
        std::fs::remove_file(small_path).expect("temporary SVG should be removed");
        let mut small_output_font_sizes: Vec<&str> = small_svg
            .split("font-size=\"")
            .skip(1)
            .filter_map(|value| value.split('\"').next())
            .collect();
        small_output_font_sizes.sort_unstable();
        small_output_font_sizes.dedup();
        assert!(!large_output_font_sizes.is_empty());
        assert_eq!(large_output_font_sizes, small_output_font_sizes);
    }

    #[test]
    fn automatic_tick_step_responds_to_available_plot_size() {
        let ticks = model::TickConfig {
            major_step: None,
            number_format: model::TickNumberFormat::Auto,
            minor_per_major: 1,
            major_ticks_visible: true,
            minor_ticks_visible: false,
            major_tick_length: 5.0,
            minor_tick_length: 3.0,
            major_grid_visible: true,
            minor_grid_visible: true,
            major_grid_color: model::Color {
                r: 190,
                g: 190,
                b: 190,
                a: 255,
            },
            major_grid_width: 1.0,
            minor_grid_color: model::Color {
                r: 225,
                g: 225,
                b: 225,
                a: 255,
            },
            minor_grid_width: 0.5,
        };
        let narrow = view::series::resolve_auto_tick_steps(
            &(0.0..5.9),
            &(0.0..10.0),
            &ticks,
            &ticks,
            (320, 240),
            40,
            80,
            30,
            view::series::ChartMargins::uniform(8),
            16,
            16,
        );
        let wide = view::series::resolve_auto_tick_steps(
            &(0.0..5.9),
            &(0.0..10.0),
            &ticks,
            &ticks,
            (1200, 800),
            40,
            80,
            30,
            view::series::ChartMargins::uniform(8),
            16,
            16,
        );
        assert!(narrow.0.major_step.unwrap() > wide.0.major_step.unwrap());
        assert!(narrow.1.major_step.unwrap() > wide.1.major_step.unwrap());
        assert!(narrow.0.major_step.unwrap() <= 5.9);
    }

    #[test]
    fn shared_label_layout_reserves_space_for_long_negative_ticks() {
        let ticks = model::TickConfig {
            major_step: Some(0.25),
            number_format: model::TickNumberFormat::Auto,
            minor_per_major: 1,
            major_ticks_visible: true,
            minor_ticks_visible: false,
            major_tick_length: 5.0,
            minor_tick_length: 3.0,
            major_grid_visible: true,
            minor_grid_visible: true,
            major_grid_color: model::Color {
                r: 190,
                g: 190,
                b: 190,
                a: 255,
            },
            major_grid_width: 1.0,
            minor_grid_color: model::Color {
                r: 225,
                g: 225,
                b: 225,
                a: 255,
            },
            minor_grid_width: 0.5,
        };
        let short = view::series::resolve_label_areas(
            &(-1.0..1.0),
            &ticks,
            model::ScaleType::Linear,
            crate::font::SANS,
            "X",
            "Y",
            30,
            30,
            14,
            14,
            16,
            16,
        );
        let long_ticks = model::TickConfig {
            major_step: Some(500_000.0),
            ..ticks.clone()
        };
        let long = view::series::resolve_label_areas(
            &(-1_000_000.0..1_000_000.0),
            &long_ticks,
            model::ScaleType::Linear,
            crate::font::SANS,
            "X",
            "Y",
            30,
            30,
            14,
            14,
            16,
            16,
        );
        let wide_values = view::series::resolve_label_areas(
            &(-1.0..1.0),
            &ticks,
            model::ScaleType::Linear,
            crate::font::SANS,
            "X",
            "Y",
            30,
            30,
            14,
            28,
            16,
            24,
        );
        assert!(long.1 > short.1);
        assert!(wide_values.1 > short.1);
    }

    #[test]
    fn undo_and_redo_restore_axis_legend_and_series_edits() {
        let mut fixture = demo_fixture();
        let series_id = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTitleAnchor {
                axis: model::AxisKind::X,
                anchor: Some([0.35, 0.8]),
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendAnchor([0.4, 0.2]))
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetLegendLayout {
                padding: 16,
                row_spacing: 12,
                sample_length: 56,
                columns: 2,
                column_spacing: 32,
                background_color: model::Color {
                    r: 250,
                    g: 240,
                    b: 220,
                    a: 210,
                },
                border_color: model::Color {
                    r: 30,
                    g: 40,
                    b: 50,
                    a: 255,
                },
                border_width: 2.0,
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesLineVisible {
                series_id,
                visible: false,
            })
            .unwrap();

        for _ in 0..4 {
            fixture
                .controller
                .dispatch(controller::action::Action::Undo)
                .unwrap();
        }
        assert!(fixture.controller.model.axes.x.title_anchor.is_none());
        assert_ne!(fixture.controller.model.legend.anchor, [0.4, 0.2]);
        assert_eq!(fixture.controller.model.legend.padding, 8);
        assert!(fixture.controller.model.series[0].style.line_visible);

        for _ in 0..4 {
            fixture
                .controller
                .dispatch(controller::action::Action::Redo)
                .unwrap();
        }
        assert_eq!(
            fixture.controller.model.axes.x.title_anchor,
            Some([0.35, 0.8])
        );
        assert_eq!(fixture.controller.model.legend.anchor, [0.4, 0.2]);
        assert_eq!(fixture.controller.model.legend.padding, 16);
        assert_eq!(fixture.controller.model.legend.sample_length, 56);
        assert_eq!(fixture.controller.model.legend.columns, 2);
        assert_eq!(fixture.controller.model.legend.column_spacing, 32);
        assert!(!fixture.controller.model.series[0].style.line_visible);
    }

    #[test]
    fn no_op_edit_preserves_redo_and_does_not_add_history() {
        let mut controller = PlotController::new();
        let original_title = controller.model.layout.title.clone();
        controller
            .dispatch(controller::action::Action::SetChartTitle(
                "Changed title".to_owned(),
            ))
            .unwrap();
        controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(controller.redo_stack.len(), 1);
        let undo_len = controller.undo_stack.len();

        controller
            .dispatch(controller::action::Action::SetChartTitle(
                original_title.clone(),
            ))
            .unwrap();

        assert_eq!(controller.model.layout.title, original_title);
        assert_eq!(controller.undo_stack.len(), undo_len);
        assert_eq!(controller.redo_stack.len(), 1);
        controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(controller.model.layout.title, "Changed title");
    }

    #[test]
    fn continuous_edit_commits_one_undo_step_after_multiple_updates() {
        let mut controller = PlotController::new();
        controller
            .dispatch(controller::action::Action::SetChartTitle(
                "draft".to_owned(),
            ))
            .unwrap();
        controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(controller.redo_stack.len(), 1);
        let original_legend = controller.model.legend.clone();
        let undo_len = controller.undo_stack.len();

        controller
            .dispatch(controller::action::Action::BeginContinuousEdit)
            .unwrap();
        for border_width in [2.0, 4.0, 7.0] {
            controller
                .dispatch(controller::action::Action::SetLegendLayout {
                    padding: controller.model.legend.padding,
                    row_spacing: controller.model.legend.row_spacing,
                    sample_length: controller.model.legend.sample_length,
                    columns: controller.model.legend.columns,
                    column_spacing: controller.model.legend.column_spacing,
                    background_color: controller.model.legend.background_color,
                    border_color: controller.model.legend.border_color,
                    border_width,
                })
                .unwrap();
        }
        assert_eq!(controller.undo_stack.len(), undo_len);
        assert_eq!(controller.redo_stack.len(), 1);
        controller
            .dispatch(controller::action::Action::EndContinuousEdit)
            .unwrap();

        assert_eq!(controller.undo_stack.len(), undo_len + 1);
        assert!(controller.redo_stack.is_empty());
        assert_eq!(controller.model.legend.border_width, 7.0);
        controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(controller.model.legend, original_legend);
        controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(controller.model.legend.border_width, 7.0);
    }

    #[test]
    fn tick_number_format_is_undoable() {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTickNumberFormat {
                axis: model::AxisKind::Y,
                format: model::TickNumberFormat::Fixed { decimals: 3 },
            })
            .unwrap();
        assert_eq!(
            fixture.controller.model.axes.y.ticks.number_format,
            model::TickNumberFormat::Fixed { decimals: 3 }
        );
        fixture
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert_eq!(
            fixture.controller.model.axes.y.ticks.number_format,
            model::TickNumberFormat::Auto
        );
        fixture
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert_eq!(
            fixture.controller.model.axes.y.ticks.number_format,
            model::TickNumberFormat::Fixed { decimals: 3 }
        );
    }

    #[test]
    fn title_visibility_and_series_marker_style_are_undoable() {
        let mut fixture = demo_fixture();
        let series_id = fixture.controller.model.series[0].id;
        fixture
            .controller
            .dispatch(controller::action::Action::SetChartTitleVisible(false))
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesMarker {
                series_id,
                marker: Some(model::MarkerShape::Cross),
                size: 7.0,
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesLineStyle {
                series_id,
                line_style: model::LineStyle::Dotted,
            })
            .unwrap();
        fixture
            .controller
            .dispatch(controller::action::Action::SetSeriesLineInterpolation {
                series_id,
                interpolation: model::LineInterpolation::StepAfter,
            })
            .unwrap();

        for _ in 0..4 {
            fixture
                .controller
                .dispatch(controller::action::Action::Undo)
                .unwrap();
        }
        assert!(fixture.controller.model.layout.title_visible);
        assert!(fixture.controller.model.series[0].style.marker.is_none());
        assert_eq!(
            fixture.controller.model.series[0].style.line_style,
            model::LineStyle::Solid
        );
        assert_eq!(
            fixture.controller.model.series[0].style.line_interpolation,
            model::LineInterpolation::Linear
        );

        for _ in 0..4 {
            fixture
                .controller
                .dispatch(controller::action::Action::Redo)
                .unwrap();
        }
        assert!(!fixture.controller.model.layout.title_visible);
        assert_eq!(
            fixture.controller.model.series[0]
                .style
                .marker
                .as_ref()
                .map(|marker| (marker.shape, marker.size)),
            Some((model::MarkerShape::Cross, 7.0))
        );
        assert_eq!(
            fixture.controller.model.series[0].style.line_style,
            model::LineStyle::Dotted
        );
        assert_eq!(
            fixture.controller.model.series[0].style.line_interpolation,
            model::LineInterpolation::StepAfter
        );
    }

    #[test]
    fn axis_tick_mark_settings_are_undoable() {
        let mut fixture = demo_fixture();
        fixture
            .controller
            .dispatch(controller::action::Action::SetAxisTickMarkStyle {
                axis: model::AxisKind::X,
                minor: false,
                visible: false,
                length: 9.0,
            })
            .unwrap();
        assert!(!fixture.controller.model.axes.x.ticks.major_ticks_visible);
        assert_eq!(fixture.controller.model.axes.x.ticks.major_tick_length, 9.0);

        fixture
            .controller
            .dispatch(controller::action::Action::Undo)
            .unwrap();
        assert!(fixture.controller.model.axes.x.ticks.major_ticks_visible);
        assert_eq!(fixture.controller.model.axes.x.ticks.major_tick_length, 5.0);
        fixture
            .controller
            .dispatch(controller::action::Action::Redo)
            .unwrap();
        assert!(!fixture.controller.model.axes.x.ticks.major_ticks_visible);
        assert_eq!(fixture.controller.model.axes.x.ticks.major_tick_length, 9.0);
    }

    #[test]
    fn step_after_interpolation_adds_horizontal_then_vertical_segments() {
        assert_eq!(
            view::series::step_after_points(&[(1.0, 2.0), (3.0, 5.0), (4.0, -1.0)]),
            vec![(1.0, 2.0), (3.0, 2.0), (3.0, 5.0), (4.0, 5.0), (4.0, -1.0),]
        );
    }

    #[test]
    fn legend_layout_dimensions_respond_to_spacing_and_sample_settings() {
        let labels = ["A", "A long experiment label"];
        let compact = view::series::legend_layout(Some("Legend"), &labels, 16, 4, 2, 12, 1, 0);
        let padded = view::series::legend_layout(Some("Legend"), &labels, 16, 16, 12, 48, 1, 0);
        let columns = view::series::legend_layout(Some("Legend"), &labels, 16, 8, 4, 24, 2, 32);
        let tight_columns =
            view::series::legend_layout(Some("Legend"), &labels, 16, 8, 4, 24, 2, 0);
        assert!(padded.width > compact.width);
        assert!(padded.height > compact.height);
        assert_eq!(columns.rows_per_column, 1);
        assert_eq!(columns.column_offsets.len(), 2);
        assert!(columns.width > compact.width);
        assert!(columns.height < compact.height);
        assert!(columns.width > tight_columns.width);
    }

    #[test]
    fn outside_legend_reserves_space_on_the_selected_side() {
        let fixture = demo_fixture();
        let labels = ["Series 1", "Second Series"];
        let mut legend = fixture.controller.model.legend.clone();
        legend.visible = true;
        for (position, side) in [
            (model::LegendPosition::OutsideLeft, "left"),
            (model::LegendPosition::OutsideRight, "right"),
            (model::LegendPosition::OutsideTop, "top"),
            (model::LegendPosition::OutsideBottom, "bottom"),
        ] {
            legend.position = position;
            let margins = view::series::chart_margins_for_legend(8, &legend, &labels, 24, 40);
            match side {
                "left" => assert!(margins.left > margins.right),
                "right" => assert!(margins.right > margins.left),
                "top" => assert!(margins.top > margins.bottom),
                "bottom" => assert!(margins.bottom > margins.top),
                _ => unreachable!(),
            }
        }
    }
}
