//! Shared UI theme.
//!
//! The app uses a single accent color (purple) for all interactive chrome:
//! header tab selection, the AI button, and the usage progress bar.

use eframe::egui;

/// Purple accent used across the app.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(124, 92, 255);

/// Launch (= minimum) window size in points.
pub const WINDOW_MIN_SIZE: [f32; 2] = [700.0, 490.0];

/// Panel minimums are sized so the headers never clip at the minimum
/// window width: assistant cap + preview minimum must fit in the window
/// (see the test below).
pub const ASSISTANT_MIN_WIDTH: f32 = 240.0;
pub const ASSISTANT_DEFAULT_WIDTH: f32 = 320.0;
pub const PREVIEW_MIN_WIDTH: f32 = 180.0;

/// Minimum width kept for the central editor so side panels can never be
/// dragged wide enough to hide it entirely.
pub const MIN_EDITOR_WIDTH: f32 = 280.0;

/// Floor for computed side-panel width caps (keeps them sane on tiny windows).
pub const MIN_SIDE_PANEL_WIDTH: f32 = 200.0;

/// Small centered X button for panel headers. Plain ASCII so it renders in
/// any font; fixed size keeps the glyph centered.
pub fn header_close_button(ui: &mut egui::Ui, hover: &str) -> egui::Response {
    ui.add_sized([22.0, 22.0], egui::Button::new("X"))
        .on_hover_text(hover)
}

/// Shared frame for all main panels (Assistant / Editor / Preview) so
/// padding and margins match exactly. (Rust has no inheritance, so the
/// panels share these helpers instead of a base class.)
pub fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::central_panel(&ctx.style())
}

/// Shared panel title row: strong label + right-aligned close button.
/// Returns the close button response; close the panel when clicked.
pub fn panel_header(ui: &mut egui::Ui, title: &str, close_hover: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            header_close_button(ui, close_hover)
        })
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_panels_fit_min_window() {
        // The assistant cap must leave room for the preview minimum, so a
        // narrow window squeezes the editor (remainder) instead of clipping
        // panel headers.
        let assistant_cap = (WINDOW_MIN_SIZE[0] - MIN_EDITOR_WIDTH).max(MIN_SIDE_PANEL_WIDTH);
        assert!(
            assistant_cap + PREVIEW_MIN_WIDTH <= WINDOW_MIN_SIZE[0],
            "panels don't fit min window: cap {assistant_cap} + preview {PREVIEW_MIN_WIDTH} > {}",
            WINDOW_MIN_SIZE[0]
        );
        assert!(ASSISTANT_MIN_WIDTH <= WINDOW_MIN_SIZE[0]);
        assert!(PREVIEW_MIN_WIDTH <= WINDOW_MIN_SIZE[0]);
    }
}
