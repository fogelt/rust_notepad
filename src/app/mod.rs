// src/app/mod.rs
use eframe::egui;
use std::fs;

pub mod models;
pub use models::NotepadApp;

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label("Filename:");
                ui.text_edit_singleline(&mut self.filename);

                if ui.button("📂 Load").clicked() {
                    match fs::read_to_string(&self.filename) {
                        Ok(text) => {
                            self.content = text;
                            self.status_message = format!("Loaded {} successfully.", self.filename);
                        }
                        Err(e) => self.status_message = format!("Error loading file: {}", e),
                    }
                }

                if ui.button("💾 Save").clicked() {
                    match fs::write(&self.filename, &self.content) {
                        Ok(_) => {
                            self.status_message =
                                format!("Saved to {} successfully.", self.filename)
                        }
                        Err(e) => self.status_message = format!("Error saving file: {}", e),
                    }
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
                        .hint_text("Type your notes here...")
                        .desired_width(f32::INFINITY),
                );
            });
        });
    }
}
