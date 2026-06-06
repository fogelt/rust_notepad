use eframe::egui;

pub mod components;
pub mod models;

pub use models::NotepadApp;

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_top_bar(ctx);
        self.show_bottom_bar(ctx);
        self.show_editor(ctx);
    }
}
