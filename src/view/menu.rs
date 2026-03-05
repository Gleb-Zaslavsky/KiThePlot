//! Top application menu widgets.
//!
//! Currently provides import/export actions via the `Files` dropdown.

use crate::controller::action::Action;
use crate::model::{ImageFormat, ImageSize};
use eframe::egui::{self, TopBottomPanel};

/// EN: Dedicated top menu module for file import/export actions.
/// RU: Otdelnyy modul verhnego menyu dlya importa/eksporta failov.
pub struct FilesMenu;

impl FilesMenu {
    /// Draws the top menu and returns generated file-related actions.
    pub fn draw(ctx: &egui::Context) -> Vec<Action> {
        let mut actions = Vec::new();

        TopBottomPanel::top("top_files_menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("Files", |ui| {
                    if ui.button("From CSV").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            actions.push(Action::ImportFromCsv {
                                path: path.display().to_string(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("From TXT").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Text", &["txt"])
                            .pick_file()
                        {
                            actions.push(Action::ImportFromTxt {
                                path: path.display().to_string(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Save as...").clicked() {
                        let size = ctx.screen_rect().size();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG image", &["png"])
                            .add_filter("SVG image", &["svg"])
                            .save_file()
                        {
                            let ext = path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_lowercase();
                            let format = if ext == "svg" {
                                ImageFormat::Svg
                            } else {
                                ImageFormat::Png
                            };
                            actions.push(Action::RequestSaveAs {
                                path: path.display().to_string(),
                                format,
                                size: ImageSize {
                                    width: size.x.max(1.0) as u32,
                                    height: size.y.max(1.0) as u32,
                                },
                            });
                        }
                        ui.close();
                    }
                });
            });
        });

        actions
    }
}


