//! Minimal host application demonstrating embedding as a library.
//!
//! Run with:
//! `cargo run --example host_app`

use kithe_plot::model::DataSource;
use kithe_plot::PlotEditorApp;

/// Simple in-memory data source owned by host app.
struct HostDataSource {
    names: Vec<String>,
    cols: Vec<Vec<f64>>,
}

impl HostDataSource {
    fn demo() -> Self {
        let x: Vec<f64> = (0..300).map(|i| i as f64 / 20.0).collect();
        let y1: Vec<f64> = x.iter().map(|v| v.sin()).collect();
        let y2: Vec<f64> = x.iter().map(|v| (v / 2.0).cos()).collect();
        Self {
            names: vec!["x".to_owned(), "sin(x)".to_owned(), "cos(x/2)".to_owned()],
            cols: vec![x, y1, y2],
        }
    }
}

impl DataSource for HostDataSource {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|idx| self.cols[idx].clone())
    }

    fn column_names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn len(&self) -> usize {
        self.cols.first().map(|c| c.len()).unwrap_or(0)
    }
}

fn main() -> Result<(), eframe::Error> {
    let source = HostDataSource::demo();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Host App + Embedded Plot Redactor",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            let mut app = PlotEditorApp::new();
            app.controller_mut()
                .load_from_data_source(&source)
                .expect("failed to load host data source");
            Ok(Box::new(app))
        }),
    )
}
