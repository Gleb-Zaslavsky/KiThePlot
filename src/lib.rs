//! Tiny, embeddable plot redactor crate.
//!
//! Architecture summary:
//! - `model`: domain data structures (`DataTable`, plot configuration, styles).
//! - `controller`: action handling, validation, undo/redo, import/export orchestration.
//! - `view`: egui UI composition and action emission.
//!
//! Dataflow:
//! `View -> Action -> Controller -> Model`, then View re-renders from model snapshot.

pub mod controller;
pub mod model;
pub mod view;

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
        Self {
            controller: PlotController::new(),
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let actions = self.view.draw(ctx, &self.controller);
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
