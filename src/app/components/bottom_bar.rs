use crate::app::NotepadApp;
use crate::app::theme::{BG_ELEVATED, BORDER};
use eframe::egui;

impl NotepadApp {
    pub fn show_bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(
                egui::Frame::none()
                    .fill(BG_ELEVATED)
                    .inner_margin(egui::Margin {
                        left: 10.0,
                        right: 10.0,
                        top: 6.0,
                        bottom: 6.0,
                    }),
            )
            .show(ctx, |ui| {
                // A hairline separates the footer from the document without
                // drawing a full border around the panel.
                ui.painter().hline(
                    ui.max_rect().x_range(),
                    ui.max_rect().top(),
                    egui::Stroke::new(1.0, BORDER),
                );

                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&self.status_message).weak())
                            .truncate(true),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(path) = &self.file_path {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.display().to_string());
                            ui.label(egui::RichText::new(name).weak().small());
                            ui.label(egui::RichText::new("·").weak().small());
                        }
                        let words = self.word_count;
                        let chars = self.char_count;
                        ui.label(
                            egui::RichText::new(format!("{words} words · {chars} chars"))
                                .weak()
                                .small(),
                        );
                    });
                });
            });
    }
}
