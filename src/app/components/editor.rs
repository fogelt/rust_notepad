use crate::app::NotepadApp;
use eframe::egui;
use egui_commonmark::CommonMarkViewer;
use std::fs;

impl NotepadApp {
    pub fn show_editor(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let visuals = ui.visuals_mut();
            visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
            visuals.selection.stroke = egui::Stroke::NONE;

            egui::ScrollArea::vertical().show(ui, |ui| {
                let max_width = ui.available_width();

                if self.preview_open {
                    ui.allocate_ui(egui::vec2(max_width, ui.available_height()), |ui| {
                        ui.add_space(2.0);
                        let mut processed_content = String::new();
                        for line in self.content.lines() {
                            processed_content.push_str(line);
                            processed_content.push_str("  \n");
                        }

                        CommonMarkViewer::new("md_viewer").show(
                            ui,
                            &mut self.markdown_cache,
                            &processed_content,
                        );
                    });
                } else {
                    let response = egui::TextEdit::multiline(&mut self.content)
                        .hint_text("Write text here...")
                        .frame(false)
                        .desired_width(max_width)
                        .layouter(&mut |ui, string, wrap_width| {
                            let mut layout_job = egui::text::LayoutJob::default();
                            layout_job.append(
                                string,
                                0.0,
                                egui::text::TextFormat {
                                    color: ui.visuals().widgets.noninteractive.text_color(),
                                    line_height: Some(18.0),
                                    ..Default::default()
                                },
                            );
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts(|f| f.layout_job(layout_job))
                        })
                        .min_size(egui::vec2(max_width, ui.available_height()))
                        .show(ui);

                    if response.response.changed() {
                        if let Some(ref path) = self.file_path {
                            if fs::write(path, &self.content).is_ok() {
                                self.status_message = String::from("Autosaved changes.");
                            } else {
                                self.status_message = String::from("Autosave failed.");
                            }
                        } else {
                            self.status_message =
                                String::from("Unsaved changes (Save file to enable autosave).");
                        }
                    }
                }
            });
        });
    }
}
