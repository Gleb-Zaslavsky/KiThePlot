//! Top application menu widgets.
//!
//! Provides import, project, and export actions via the `Files` dropdown.

use crate::controller::action::Action;
use crate::model::{
    DecimalConvention, HeaderMode, ImageFormat, ImportDelimiter, ImportOptions,
    PublicationDimensions,
};
use std::path::PathBuf;

use eframe::egui::{self, Ui};

/// EN: Dedicated top menu module for file import/export actions.
/// RU: Otdelnyy modul verhnego menyu dlya importa/eksporta failov.
pub struct FilesMenu;

impl FilesMenu {
    /// Draws the top menu and returns generated file-related actions.
    pub fn draw(
        ui: &mut Ui,
        dimensions: PublicationDimensions,
        controls_collapsed: &mut bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();

        egui::Panel::top("top_files_menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(if *controls_collapsed {
                        "Show controls"
                    } else {
                        "Hide controls"
                    })
                    .clicked()
                {
                    *controls_collapsed = !*controls_collapsed;
                }
                ui.menu_button("Files", |ui| {
                    if ui.button("Add table from CSV...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            actions.push(Action::PreviewAddTable {
                                path: path.display().to_string(),
                                options: ImportOptions::default(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Add table from TXT...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Text", &["txt"])
                            .pick_file()
                        {
                            actions.push(Action::PreviewAddTable {
                                path: path.display().to_string(),
                                options: ImportOptions {
                                    delimiter: ImportDelimiter::Whitespace,
                                    decimal: DecimalConvention::Dot,
                                    header: HeaderMode::Auto,
                                },
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Replace primary table from CSV...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            actions.push(Action::PreviewImport {
                                path: path.display().to_string(),
                                options: ImportOptions::default(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Replace primary table from TXT...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Text", &["txt"])
                            .pick_file()
                        {
                            actions.push(Action::PreviewImport {
                                path: path.display().to_string(),
                                options: ImportOptions {
                                    delimiter: ImportDelimiter::Whitespace,
                                    decimal: DecimalConvention::Dot,
                                    header: HeaderMode::Auto,
                                },
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Open project...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("KiThePlot project", &["kplot"])
                            .pick_file()
                        {
                            actions.push(Action::OpenProject {
                                path: path.display().to_string(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Save project...").clicked() {
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
                            actions.push(Action::SaveProject {
                                path: path.display().to_string(),
                                dimensions,
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Apply plot style...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("KiThePlot style", &["kstyle"])
                            .pick_file()
                        {
                            actions.push(Action::ApplyPlotStyle {
                                path: path.display().to_string(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Save plot style...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("KiThePlot style", &["kstyle"])
                            .set_file_name("Publication.kstyle")
                            .save_file()
                        {
                            let path = if path.extension().is_none() {
                                path.with_extension("kstyle")
                            } else {
                                path
                            };
                            actions.push(Action::SavePlotStyle {
                                path: path.display().to_string(),
                            });
                        }
                        ui.close();
                    }

                    if ui.button("Export figure...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG image", &["png"])
                            .add_filter("SVG image", &["svg"])
                            .add_filter("PDF document", &["pdf"])
                            .save_file()
                        {
                            match resolve_export_path(path) {
                                Ok((path, format)) => {
                                    actions.push(Action::RequestPublicationExport {
                                        path: path.display().to_string(),
                                        format,
                                        dimensions,
                                    });
                                }
                                Err(message) => actions.push(Action::ReportUiError(message)),
                            }
                        }
                        ui.close();
                    }
                });
            });
        });

        actions
    }
}

fn resolve_export_path(mut path: PathBuf) -> Result<(PathBuf, ImageFormat), String> {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        path.set_extension("png");
        return Ok((path, ImageFormat::Png));
    };

    match extension.to_ascii_lowercase().as_str() {
        "png" => Ok((path, ImageFormat::Png)),
        "svg" => Ok((path, ImageFormat::Svg)),
        "pdf" => Ok((path, ImageFormat::Pdf)),
        _ => Err(format!(
            "Unsupported export extension '.{extension}'. Choose PNG, SVG, or PDF."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_export_path;
    use crate::model::ImageFormat;
    use std::path::PathBuf;

    #[test]
    fn export_path_requires_a_supported_extension_and_adds_png_when_missing() {
        let (path, format) = resolve_export_path(PathBuf::from("figure"))
            .expect("missing extension should select PNG");
        assert_eq!(path, PathBuf::from("figure.png"));
        assert!(matches!(format, ImageFormat::Png));

        let (path, format) = resolve_export_path(PathBuf::from("figure.SvG"))
            .expect("SVG extensions should be case-insensitive");
        assert_eq!(path, PathBuf::from("figure.SvG"));
        assert!(matches!(format, ImageFormat::Svg));

        let (path, format) = resolve_export_path(PathBuf::from("figure.pdf"))
            .expect("PDF should be a supported vector export format");
        assert_eq!(path, PathBuf::from("figure.pdf"));
        assert!(matches!(format, ImageFormat::Pdf));
    }
}
