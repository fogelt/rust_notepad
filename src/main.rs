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

    let icon_bytes = include_bytes!("../assets/icon.png");
    let icon_data = if let Ok(image) = image::load_from_memory(icon_bytes) {
        let image = image.to_rgba8();
        let (width, height) = image.dimensions();
        Some(egui::IconData {
            rgba: image.into_raw(),
            width,
            height,
        })
    } else {
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([800.0, 600.0])
        .with_title("Rusty Notepad");

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        centered: true, // This field lives on NativeOptions to manage window initialization placement
        ..Default::default()
    };

    eframe::run_native(
        "rust_notepad",
        options,
        Box::new(move |_cc| Box::new(app::NotepadApp::with_file(initial_path))),
    )
}
