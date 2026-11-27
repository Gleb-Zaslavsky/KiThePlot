//! Put the plot editor inside a larger host application's existing window.
//!
//! Run with: `cargo run --example host_app_embedded`

use kithe_plot::{PlotEditorApp, model::DataSource};

struct InstrumentData {
    amplitude: f64,
}

impl DataSource for InstrumentData {
    fn column(&self, name: &str) -> Option<Vec<f64>> {
        let time: Vec<f64> = (0..240).map(|i| i as f64 * 0.025).collect();
        match name {
            "time_s" => Some(time),
            "voltage_mV" => Some(
                time.iter()
                    .map(|t| {
                        self.amplitude
                            * (-0.4 * t).exp()
                            * (2.0 * std::f64::consts::PI * 3.0 * t).sin()
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    fn column_names(&self) -> Vec<String> {
        vec!["time_s".into(), "voltage_mV".into()]
    }
}

struct LabWorkspace {
    editor: PlotEditorApp,
    run_label: String,
}

impl eframe::App for LabWorkspace {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.heading("Oscilloscope workspace");
            ui.separator();
            ui.label(&self.run_label);
            if ui.button("Next run").clicked() {
                let next_run = InstrumentData { amplitude: 8.0 };
                match self
                    .editor
                    .controller_mut()
                    .load_from_data_source(&next_run)
                {
                    Ok(()) => self.run_label = "Run B · lower amplitude".into(),
                    Err(error) => self.run_label = format!("Run update failed: {error}"),
                }
            }
        });
        ui.separator();

        // KiThePlot renders into this host-provided UI area. The surrounding
        // application keeps ownership of the window and its own controls.
        self.editor.ui(ui, frame);
    }
}

fn main() -> Result<(), eframe::Error> {
    let source = InstrumentData { amplitude: 14.0 };
    eframe::run_native(
        "Lab workspace",
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            let mut editor = PlotEditorApp::new();
            editor
                .controller_mut()
                .load_from_data_source(&source)
                .expect("load instrument data");
            editor.controller_mut().model.layout.title = "Oscilloscope trace".into();
            editor.controller_mut().model.axes.x.label = "Time, s".into();
            editor.controller_mut().model.axes.y.label = "Voltage, mV".into();
            Ok(Box::new(LabWorkspace {
                editor,
                run_label: "Run A · synthetic measurement".into(),
            }))
        }),
    )
}
