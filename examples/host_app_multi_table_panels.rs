//! Bind separate series to separate tables and give each subplot its own axes.
//!
//! Run with: `cargo run --example host_app_multi_table_panels`

use kithe_plot::{PlotEditorApp, controller::action::Action, model};

struct Measurements {
    columns: Vec<(String, Vec<f64>)>,
}

impl model::DataSource for Measurements {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        self.columns
            .iter()
            .find(|(column, _)| column == name)
            .map(|(_, values)| values.clone())
    }

    fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|(name, _)| name.clone()).collect()
    }
}

fn main() -> Result<(), eframe::Error> {
    let time: Vec<f64> = (0..160).map(|i| i as f64 * 0.05).collect();
    let measured = Measurements {
        columns: vec![
            ("time_s".into(), time.clone()),
            (
                "concentration_mM".into(),
                time.iter()
                    .map(|t| 3.8 * (-0.55 * t).exp() * (1.0 + 0.025 * (11.0 * t).sin()))
                    .collect(),
            ),
        ],
    };
    let fitted = Measurements {
        columns: vec![
            ("time_s".into(), time.clone()),
            (
                "concentration_mM".into(),
                time.iter().map(|t| 3.8 * (-0.55 * t).exp()).collect(),
            ),
            // A short auxiliary column demonstrates that unrelated columns
            // need not share the length of the plotted X/Y pair.
            ("fit_residual_summary".into(), vec![0.025, 0.019, 0.013]),
        ],
    };

    eframe::run_native(
        "KiThePlot — multiple tables and panel axes",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            let mut plot_model = model::PlotModel::default();
            plot_model.layout.title = "Measured values and fit".into();
            plot_model.axes.x.label = "Time, s".into();
            plot_model.axes.y.label = "Concentration, mM".into();
            let mut app = PlotEditorApp::with_model(plot_model);
            let controller = app.controller_mut();
            controller
                .load_from_data_source(&measured)
                .expect("load measured table");
            let fit_table = model::DataTable::from_data_source(&fitted).expect("build fit table");
            let fit_table_id = controller
                .add_data_table(fit_table, "Model fit")
                .expect("add fit table");
            controller
                .dispatch(Action::AddSeries {
                    name: "Exponential fit".into(),
                    x_column: "time_s".into(),
                    y_column: "concentration_mM".into(),
                })
                .expect("add fit series");
            let fit_series_id = controller.model.series.last().expect("fit series").id;
            controller
                .dispatch(Action::SetSeriesTable {
                    series_id: fit_series_id,
                    table_id: fit_table_id,
                })
                .expect("bind fit series to fit table");
            let measured_series_id = controller.model.series[0].id;
            controller
                .dispatch(Action::SetPanels(vec![
                    model::PlotPanel {
                        id: 1,
                        title: "Experiment".into(),
                        series_ids: vec![measured_series_id],
                        axis_view: None,
                    },
                    model::PlotPanel {
                        id: 2,
                        title: "Fit (log Y)".into(),
                        series_ids: vec![fit_series_id],
                        axis_view: Some(model::PanelAxisView {
                            x_scale: model::ScaleType::Linear,
                            y_scale: model::ScaleType::Log10Axis,
                            x_range: model::RangePolicy::Auto,
                            y_range: model::RangePolicy::Auto,
                        }),
                    },
                ]))
                .expect("configure panels");
            Ok(Box::new(app))
        }),
    )
}
