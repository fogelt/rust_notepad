use eframe::egui;
use rfd::FileDialog;
use std::fs;

pub mod models;
pub use models::NotepadApp;

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("📄 New").clicked() {
                        self.content.clear();
                        self.file_path = None;
                        self.status_message = String::from("New file created.");
                        ui.close_menu();
                    }

                    if ui.button("📂 Open...").clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("Text files", &["txt", "md"])
                            .pick_file()
                        {
                            match fs::read_to_string(&path) {
                                Ok(text) => {
                                    self.content = text;
                                    self.status_message =
                                        format!("Opened: {:?}", path.file_name().unwrap());
                                    self.file_path = Some(path);
                                }
                                Err(e) => {
                                    self.status_message = format!("Error opening file: {}", e)
                                }
                            }
                        }
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("💾 Save").clicked() {
                        if let Some(ref path) = self.file_path {
                            match fs::write(path, &self.content) {
                                Ok(_) => self.status_message = format!("Saved successfully."),
                                Err(e) => self.status_message = format!("Error saving: {}", e),
                            }
                        } else {
                            if let Some(path) = FileDialog::new().save_file() {
                                match fs::write(&path, &self.content) {
                                    Ok(_) => {
                                        self.status_message = format!("Saved successfully.");
                                        self.file_path = Some(path);
                                    }
                                    Err(e) => self.status_message = format!("Error saving: {}", e),
                                }
                            }
                        }
                        ui.close_menu();
                    }

                    if ui.button("💾 Save As...").clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("Text file", &["txt"])
                            .save_file()
                        {
                            match fs::write(&path, &self.content) {
                                Ok(_) => {
                                    self.status_message = format!("Saved to new path.");
                                    self.file_path = Some(path);
                                }
                                Err(e) => self.status_message = format!("Error saving: {}", e),
                            }
                        }
                        ui.close_menu();
                    }
                });

                ui.separator();

                if let Some(ref path) = self.file_path {
                    ui.label(format!(
                        "Editing: {}",
                        path.file_name().unwrap().to_string_lossy()
                    ));
                } else {
                    ui.label("Editing: *Untitled*");
                }
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut self.content)
                        .hint_text("Start typing your thoughts...")
                        .desired_width(f32::INFINITY),
                );
            });
        });
    }
}
