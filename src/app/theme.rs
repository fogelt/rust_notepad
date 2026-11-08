//! Shared UI theme.
//!
//! A small, neutral palette is defined once here and applied globally in
//! [`apply`]. The accent is a desaturated slate (rather than a saturated
//! purple) so the chrome stays calm and the document remains the focus.

use eframe::egui;

/// Neutral slate accent used for selected tabs, primary buttons and progress.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(98, 110, 126);
/// Readable accent-tinted color for links on the dark background.
pub const ACCENT_LINK: egui::Color32 = egui::Color32::from_rgb(126, 156, 200);
/// Text drawn on top of [`ACCENT`] fills.
pub const ACCENT_TEXT: egui::Color32 = egui::Color32::WHITE;

/// Main panel background.
pub const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(28, 29, 32);
/// Slightly elevated surface used by the header/footer bars.
pub const BG_ELEVATED: egui::Color32 = egui::Color32::from_rgb(33, 34, 38);
/// Input / code-block background.
pub const BG_INPUT: egui::Color32 = egui::Color32::from_rgb(20, 21, 24);
/// Subtle alternating-row background.
pub const BG_FAINT: egui::Color32 = egui::Color32::from_rgb(36, 37, 42);
/// Fenced code block background.
pub const BG_CODE: egui::Color32 = egui::Color32::from_rgb(21, 22, 26);
/// Default border color.
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(52, 54, 60);
/// Stronger border used on hover.
pub const BORDER_STRONG: egui::Color32 = egui::Color32::from_rgb(74, 77, 86);
/// Primary text.
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(228, 228, 231);
/// Button / surface background.
pub const SURFACE: egui::Color32 = egui::Color32::from_rgb(42, 44, 49);
/// Hovered button / surface background.
pub const SURFACE_HOVERED: egui::Color32 = egui::Color32::from_rgb(52, 55, 61);
/// Pressed button / surface background.
pub const SURFACE_ACTIVE: egui::Color32 = egui::Color32::from_rgb(62, 65, 72);

/// Launch (= minimum) window size in points.
pub const WINDOW_MIN_SIZE: [f32; 2] = [700.0, 490.0];

/// Panel minimums are sized so the headers never clip at the minimum
/// window width: assistant cap + preview minimum must fit in the window
/// (see the test below).
pub const ASSISTANT_MIN_WIDTH: f32 = 240.0;
pub const ASSISTANT_DEFAULT_WIDTH: f32 = 320.0;
pub const PREVIEW_MIN_WIDTH: f32 = 180.0;

/// Comfortable reading measure for the preview, in points. Full-width
/// previews center their content instead of stretching text edge to edge.
pub const PREVIEW_MAX_CONTENT_WIDTH: f32 = 720.0;

/// Minimum width kept for the central editor so side panels can never be
/// dragged wide enough to hide it entirely.
pub const MIN_EDITOR_WIDTH: f32 = 280.0;

/// Floor for computed side-panel width caps (keeps them sane on tiny windows).
pub const MIN_SIDE_PANEL_WIDTH: f32 = 200.0;

/// Install the app-wide look once at startup.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_ELEVATED;
    visuals.extreme_bg_color = BG_INPUT;
    visuals.faint_bg_color = BG_FAINT;
    visuals.code_bg_color = BG_CODE;
    visuals.hyperlink_color = ACCENT_LINK;
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.window_rounding = egui::Rounding::same(8.0);
    visuals.menu_rounding = egui::Rounding::same(6.0);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = egui::Stroke::NONE;
    visuals.error_fg_color = egui::Color32::from_rgb(230, 118, 118);
    visuals.warn_fg_color = egui::Color32::from_rgb(226, 178, 94);
    visuals.striped = true;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = BG_PANEL;
    widgets.noninteractive.weak_bg_fill = BG_PANEL;
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    widgets.noninteractive.rounding = egui::Rounding::same(6.0);

    widgets.inactive.bg_fill = SURFACE;
    widgets.inactive.weak_bg_fill = SURFACE;
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    widgets.inactive.rounding = egui::Rounding::same(6.0);

    widgets.hovered.bg_fill = SURFACE_HOVERED;
    widgets.hovered.weak_bg_fill = SURFACE_HOVERED;
    widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BORDER_STRONG);
    widgets.hovered.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
    widgets.hovered.rounding = egui::Rounding::same(6.0);

    widgets.active.bg_fill = SURFACE_ACTIVE;
    widgets.active.weak_bg_fill = SURFACE_ACTIVE;
    widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    widgets.active.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
    widgets.active.rounding = egui::Rounding::same(6.0);

    widgets.open.bg_fill = SURFACE_ACTIVE;
    widgets.open.weak_bg_fill = SURFACE_ACTIVE;
    widgets.open.bg_stroke = egui::Stroke::new(1.0, BORDER_STRONG);
    widgets.open.fg_stroke = egui::Stroke::new(1.0, TEXT);
    widgets.open.rounding = egui::Rounding::same(6.0);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.scroll = egui::style::ScrollStyle::thin();
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.5));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));

    ctx.set_style(style);
}

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
        ui.label(egui::RichText::new(title).strong().size(14.0));
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
    }
}
