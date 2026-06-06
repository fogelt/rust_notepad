use crate::app::NotepadApp;
use eframe::egui;

impl NotepadApp {
    pub fn show_bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin {
                    left: 5.0,
                    right: 5.0,
                    top: 8.0,
                    bottom: 6.0,
                })
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.status_message).weak());
                });
        });
    }
}
