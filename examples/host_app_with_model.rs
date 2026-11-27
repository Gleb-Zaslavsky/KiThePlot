//! Start from host-provided plot defaults, then let the editor handle the data.
//!
//! Run with: `cargo run --example host_app_with_model`

use kithe_plot::{PlotEditorApp, model::DataSource};

struct KineticsData;

impl DataSource for KineticsData {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        let time: Vec<f64> = (0..180).map(|i| i as f64 * 0.05).collect();
        match name {
            "time_s" => Some(time),
            "concentration_mM" => Some(time.iter().map(|t| 4.0 * (-0.7 * t).exp()).collect()),
            _ => None,
        }
    }

    fn column_names(&self) -> Vec<String> {
        vec!["time_s".into(), "concentration_mM".into()]
    }
}

fn main() -> Result<(), eframe::Error> {
    let mut model = kithe_plot::model::PlotModel::default();
    model.layout.title = "First-order decay".into();
    model.layout.title_visible = true;
    model.axes.x.label = "Time, s".into();
    model.axes.y.label = "Concentration, mM".into();
    model.legend.title = Some("Kinetic run".into());

    let source = KineticsData;
    eframe::run_native(
        "KiThePlot — host-provided plot defaults",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            let mut app = PlotEditorApp::with_model(model);
            app.controller_mut()
                .load_from_data_source(&source)
                .expect("load kinetics data");
            Ok(Box::new(app))
        }),
    )
}
