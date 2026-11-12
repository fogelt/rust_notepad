//! Shared UI theme.
//!
//! A small, calm palette is defined once here and applied globally in
//! [`apply`]. The accent defaults to a soft blue, but can follow the active
//! provider's branding color when the user enables that setting.

use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Vivid punk yellow used as the custom accent until the user picks a color.
pub const PUNK_YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 230, 0);
/// Soft rainbow stops used when the active provider asks for a gradient. The
/// first color is repeated at the end so neighbouring tabs join seamlessly.
const RAINBOW: [egui::Color32; 7] = [
    egui::Color32::from_rgb(255, 154, 162),
    egui::Color32::from_rgb(255, 176, 124),
    egui::Color32::from_rgb(253, 253, 150),
    egui::Color32::from_rgb(168, 230, 163),
    egui::Color32::from_rgb(160, 196, 255),
    egui::Color32::from_rgb(205, 164, 255),
    egui::Color32::from_rgb(255, 154, 162),
];
static ACCENT_RGB: AtomicU32 = AtomicU32::new(0xFF_E6_00);
static ACCENT_GRADIENT: AtomicBool = AtomicBool::new(false);

/// Set the accent used by accent-colored chrome. Call [`apply`] afterwards to
/// refresh egui's built-in visuals.
pub fn set_accent(color: egui::Color32) {
    let [r, g, b, _] = color.to_array();
    ACCENT_RGB.store(
        ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        Ordering::Relaxed,
    );
    ACCENT_GRADIENT.store(false, Ordering::Relaxed);
}

/// Set a solid base accent that accent *fills* render as a rainbow gradient.
pub fn set_accent_gradient(base: egui::Color32) {
    set_accent(base);
    ACCENT_GRADIENT.store(true, Ordering::Relaxed);
}

/// Whether accent fills are currently drawn as a rainbow gradient.
pub fn accent_is_gradient() -> bool {
    ACCENT_GRADIENT.load(Ordering::Relaxed)
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

fn mix(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn gradient_color(t: f32, hovered: bool) -> egui::Color32 {
    let last = (RAINBOW.len() - 1) as f32;
    let pos = (t.clamp(0.0, 1.0) * last).min(last - 0.001);
    let index = pos.floor() as usize;
    let color = mix(RAINBOW[index], RAINBOW[index + 1], pos - index as f32);
    if hovered { lighten(color, 12) } else { color }
}

/// Vertical extent of a rounded rectangle at `x`, used to clip the gradient
/// columns to the rounded corners.
fn rounded_span(rect: egui::Rect, radius: f32, x: f32) -> (f32, f32) {
    let dx = if x < rect.left() + radius {
        rect.left() + radius - x
    } else if x > rect.right() - radius {
        x - (rect.right() - radius)
    } else {
        0.0
    };
    let dy = (radius * radius - dx * dx).max(0.0).sqrt();
    (rect.top() + radius - dy, rect.bottom() - radius + dy)
}

/// Paint an accent fill, either as a solid color or as the provider's rainbow
/// gradient.
pub fn paint_accent_fill(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::Rounding,
    hovered: bool,
) {
    if !accent_is_gradient() {
        painter.rect_filled(
            rect,
            rounding,
            if hovered { accent_hovered() } else { accent() },
        );
        return;
    }

    let radius = rounding
        .nw
        .min(rounding.ne)
        .min(rounding.sw)
        .min(rounding.se)
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    // Soft edge for the rounded corners, hidden under the gradient itself.
    painter.rect_filled(rect, rounding, gradient_color(0.5, hovered));

    // Narrow columns keep interpolation between neighbouring gradient
    // positions, so the sRGB/linear conversion does not desaturate the hues.
    let columns = 64;
    let width = rect.width().max(1.0);
    let mut mesh = egui::Mesh::default();
    for i in 0..columns {
        let x0 = rect.left() + width * i as f32 / columns as f32;
        let x1 = rect.left() + width * (i + 1) as f32 / columns as f32;
        let (top0, bottom0) = rounded_span(rect, radius, x0);
        let (top1, bottom1) = rounded_span(rect, radius, x1);
        let color0 = gradient_color((x0 - rect.left()) / width, hovered);
        let color1 = gradient_color((x1 - rect.left()) / width, hovered);
        let base = mesh.vertices.len() as u32;
        for (pos, color) in [
            (egui::pos2(x0, top0), color0),
            (egui::pos2(x1, top1), color1),
            (egui::pos2(x1, bottom1), color1),
            (egui::pos2(x0, bottom0), color0),
        ] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos,
                uv: egui::epaint::WHITE_UV,
                color,
            });
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
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
