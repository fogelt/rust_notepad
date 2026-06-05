#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Rusty notepad"),
        ..Default::default()
    };

    eframe::run_native(
        "rust_notepad",
        options,
        Box::new(|_cc| Box::new(app::NotepadApp::default())),
    )
}
