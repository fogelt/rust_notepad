use crate::app::NotepadApp;
use crate::app::theme::ACCENT;
use eframe::egui;
use rfd::FileDialog;
use std::fs;

/// Header tab with a min-width so the title always fits; click toggles a panel.
/// Each panel header has its own × close button.
fn view_tab(ui: &mut egui::Ui, selected: &mut bool, label: &str, hover: &str) {
    if ui
        .add_sized(
            egui::vec2(88.0, 24.0),
            egui::SelectableLabel::new(*selected, label),
        )
        .on_hover_text(hover)
        .clicked()
    {
        *selected = !*selected;
    }
}

impl NotepadApp {
    pub fn show_top_bar(&mut self, ctx: &egui::Context) {
        let mut action_new = false;
        let mut action_open = false;
        let mut action_save = false;

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(8.0, 4.0)) // Adds clean padding to the bar
                .show(ui, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("New").clicked() {
                                action_new = true;
                                ui.close_menu();
                            }
                            if ui.button("Open...").clicked() {
                                action_open = true;
                                ui.close_menu();
                            }
                            if ui.button("Save").clicked() {
                                action_save = true;
                                ui.close_menu();
                            }
                        });
                        ui.separator();
                        // Single app accent color for tab selection.
                        ui.visuals_mut().selection.bg_fill = ACCENT;
                        // Order mirrors the panel layout: assistant (left),
                        // editor (center), preview (right).
                        view_tab(
                            ui,
                            &mut self.tabs.assistant,
                            "Assistant",
                            "Toggle the Gemini assistant panel",
                        );
                        view_tab(
                            ui,
                            &mut self.tabs.editor,
                            "Editor",
                            "Toggle the editor panel",
                        );
                        view_tab(
                            ui,
                            &mut self.tabs.preview,
                            "Preview",
                            "Toggle the Markdown preview panel",
                        );
                    });
                });
        });

        if action_new {
            self.content.clear();
            self.file_path = None;
            self.status_message = String::from("New file created.");
        }

        if action_open {
            if let Some(path) = FileDialog::new()
                .add_filter("Markdown/Text", &["md", "txt"])
                .pick_file()
            {
                if let Ok(text) = fs::read_to_string(&path) {
                    self.content = text;
                    self.file_path = Some(path);
                    self.status_message = String::from("Opened file successfully.");
                }
            }
        }

        if action_save {
            if self.file_path.is_none() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Markdown/Text", &["md", "txt"])
                    .save_file()
                {
                    self.file_path = Some(path);
                }
            }

            if let Some(ref path) = self.file_path {
                if fs::write(path, &self.content).is_ok() {
                    self.status_message = String::from("Saved.");
                } else {
                    self.status_message = String::from("Failed to save file.");
                }
            }
        }
    }
}
