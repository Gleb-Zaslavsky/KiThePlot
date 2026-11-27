//! Provide columns with independent lengths, as can happen after resampling.
//!
//! The valid `time`/`signal` pair is plotted in full; the shorter diagnostic
//! column remains available without truncating the signal.
//! Run with: `cargo run --example host_app_ragged`

use kithe_plot::{PlotEditorApp, model::DataSource};

struct ResampledRun {
    time: Vec<f64>,
    signal: Vec<f64>,
    diagnostic: Vec<f64>,
}

impl ResampledRun {
    fn demo() -> Self {
        let time: Vec<f64> = (0..500).map(|i| i as f64 * 0.01).collect();
        let signal = time.iter().map(|t| (-t).exp() * (8.0 * t).sin()).collect();
        let diagnostic = (0..120).map(|i| 0.02 + i as f64 * 0.001).collect();
        Self {
            time,
            signal,
            diagnostic,
        }
    }
}

impl DataSource for ResampledRun {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        match name {
            "time_s" => Some(self.time.clone()),
            "signal_mV" => Some(self.signal.clone()),
            "baseline_check" => Some(self.diagnostic.clone()),
            _ => None,
        }
    }

    fn column_names(&self) -> Vec<String> {
        vec!["time_s".into(), "signal_mV".into(), "baseline_check".into()]
    }
}

fn main() -> Result<(), eframe::Error> {
    let source = ResampledRun::demo();
    eframe::run_native(
        "KiThePlot — ragged host data",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            let mut app = PlotEditorApp::new();
            app.controller_mut()
                .load_from_data_source(&source)
                .expect("load resampled data");
            Ok(Box::new(app))
        }),
    )
}
