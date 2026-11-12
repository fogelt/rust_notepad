#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use eframe::egui;
use std::path::PathBuf;

mod app;
mod rmd_cli;

fn main() -> eframe::Result<()> {
    // The AppImage wrapper launches the GUI with `--rmd` when the bundled
    // `rmd` command is used (the real rmd binary is inside the AppImage).
    if std::env::args().nth(1).as_deref() == Some("--rmd") {
        std::process::exit(rmd_cli::run_with(std::env::args().skip(2)));
    }

    // Load GEMINI_API_KEY (and optional GEMINI_MODEL) from .env; ignored if absent.
    let _ = dotenvy::dotenv();
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
        .with_inner_size(crate::app::theme::WINDOW_MIN_SIZE)
        .with_min_inner_size(crate::app::theme::WINDOW_MIN_SIZE)
        .with_drag_and_drop(true)
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
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            crate::app::theme::apply(&cc.egui_ctx);
            Box::new(app::NotepadApp::with_file(initial_path))
        }),
    )
}
