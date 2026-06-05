#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::path::PathBuf;

mod app;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Rust egui Notepad"),
        ..Default::default()
    };

    eframe::run_native(
        "rust_notepad",
        options,
        Box::new(move |_cc| Box::new(app::NotepadApp::with_file(initial_path))),
    )
}
