//! Shared UI theme.
//!
//! A small, calm palette is defined once here and applied globally in
//! [`apply`]. The accent defaults to a soft blue, but can follow the active
//! provider's branding color when the user enables that setting.

use eframe::egui;
use std::sync::atomic::{AtomicU32, Ordering};

/// Default calm blue accent used for selected tabs, primary buttons and
/// progress.
pub const DEFAULT_ACCENT: egui::Color32 = egui::Color32::from_rgb(74, 120, 173);
static ACCENT_RGB: AtomicU32 = AtomicU32::new(0x4A_78_AD);

/// Set the accent used by accent-colored chrome. Call [`apply`] afterwards to
/// refresh egui's built-in visuals.
pub fn set_accent(color: egui::Color32) {
    let [r, g, b, _] = color.to_array();
    ACCENT_RGB.store(
        ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        Ordering::Relaxed,
    );
}

/// Current accent color.
pub fn accent() -> egui::Color32 {
    let value = ACCENT_RGB.load(Ordering::Relaxed);
    egui::Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

fn lighten(color: egui::Color32, amount: u8) -> egui::Color32 {
    let add = |channel: u8| channel.saturating_add(amount);
    egui::Color32::from_rgb(add(color.r()), add(color.g()), add(color.b()))
}

/// Lighter accent for hovered accent-colored controls.
pub fn accent_hovered() -> egui::Color32 {
    lighten(accent(), 16)
}

/// Readable accent-tinted color for links on the dark background.
pub fn accent_link() -> egui::Color32 {
    lighten(accent(), 52)
}

/// Text drawn on top of [`accent`] fills. Light brand colors get dark text.
pub fn accent_text() -> egui::Color32 {
    let color = accent();
    let luminance =
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    if luminance > 145.0 {
        egui::Color32::from_rgb(18, 20, 24)
    } else {
        egui::Color32::WHITE
    }
}

/// Duration of the shared chevron flip animation, in seconds.
pub const CHEVRON_ANIMATION_TIME: f32 = 0.16;

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
    visuals.hyperlink_color = accent_link();
    visuals.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    visuals.window_rounding = egui::Rounding::same(8.0);
    visuals.menu_rounding = egui::Rounding::same(6.0);
    visuals.selection.bg_fill = accent();
    // A zero-width stroke still supplies the selected-text color (white)
    // without drawing a border around selected labels/buttons.
    visuals.selection.stroke = egui::Stroke::new(0.0_f32, accent_text());
    visuals.text_cursor = egui::Stroke::new(1.0_f32, accent());
    visuals.error_fg_color = egui::Color32::from_rgb(230, 118, 118);
    visuals.warn_fg_color = egui::Color32::from_rgb(226, 178, 94);
    visuals.striped = true;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = BG_PANEL;
    widgets.noninteractive.weak_bg_fill = BG_PANEL;
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    widgets.noninteractive.rounding = egui::Rounding::same(6.0);

    widgets.inactive.bg_fill = SURFACE;
    widgets.inactive.weak_bg_fill = SURFACE;
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    widgets.inactive.rounding = egui::Rounding::same(6.0);

    widgets.hovered.bg_fill = SURFACE_HOVERED;
    widgets.hovered.weak_bg_fill = SURFACE_HOVERED;
    widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_STRONG);
    widgets.hovered.fg_stroke = egui::Stroke::new(1.5_f32, egui::Color32::WHITE);
    widgets.hovered.rounding = egui::Rounding::same(6.0);

    widgets.active.bg_fill = SURFACE_ACTIVE;
    widgets.active.weak_bg_fill = SURFACE_ACTIVE;
    widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, accent());
    widgets.active.fg_stroke = egui::Stroke::new(1.5_f32, egui::Color32::WHITE);
    widgets.active.rounding = egui::Rounding::same(6.0);

    widgets.open.bg_fill = SURFACE_ACTIVE;
    widgets.open.weak_bg_fill = SURFACE_ACTIVE;
    widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, BORDER_STRONG);
    widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
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

/// Draw a small chevron pointing down at `flip = 0` and up at `flip = 1`.
/// Shared by the Settings menu, collapsible headers, and combo boxes.
pub fn draw_chevron(ui: &egui::Ui, center: egui::Pos2, flip: f32, color: egui::Color32) {
    let angle = std::f32::consts::PI * flip;
    let (sin, cos) = angle.sin_cos();
    let local = [
        egui::vec2(-4.0, -1.5),
        egui::vec2(0.0, 2.5),
        egui::vec2(4.0, -1.5),
    ];
    let rotate = |point: egui::Vec2| {
        egui::pos2(
            center.x + point.x * cos - point.y * sin,
            center.y + point.x * sin + point.y * cos,
        )
    };
    let points = [rotate(local[0]), rotate(local[1]), rotate(local[2])];
    let stroke = egui::Stroke::new(1.6_f32, color);
    ui.painter().line_segment([points[0], points[1]], stroke);
    ui.painter().line_segment([points[1], points[2]], stroke);
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
