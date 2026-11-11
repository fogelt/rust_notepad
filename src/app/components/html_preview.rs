use eframe::egui;
use litehtml::pixbuf::PixbufContainer;
use litehtml::{Document, DrawContext, Position};
use std::collections::HashMap;
use std::path::Path;

/// Rasterized HTML preview. Rendering is cached until the source, width or
/// display scale changes.
pub struct HtmlPreview {
    texture: Option<egui::TextureHandle>,
    size: egui::Vec2,
    key: Option<Key>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    source_hash: u64,
    width: u32,
    scale_milli: u32,
}

impl HtmlPreview {
    pub fn new() -> Self {
        Self {
            texture: None,
            size: egui::Vec2::ZERO,
            key: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, html: &str, source_hash: u64, width: f32) {
        let scale = ui.ctx().pixels_per_point();
        let key = Key {
            source_hash,
            width: width.round().max(1.0) as u32,
            scale_milli: (scale * 1000.0).round() as u32,
        };
        if self.key != Some(key) {
            self.key = Some(key);
            self.texture = None;
            if let Some((texture, size)) = render(ui.ctx(), html, width, scale, source_hash) {
                self.texture = Some(texture);
                self.size = size;
            }
        }

        if let Some(texture) = &self.texture {
            ui.add(egui::Image::new(texture).fit_to_exact_size(self.size));
        } else {
            ui.weak("Could not render the HTML preview.");
        }
    }
}

/// Render `html` at `width` logical points. Returns the texture and its logical
/// display size.
fn render(
    ctx: &egui::Context,
    html: &str,
    width: f32,
    scale: f32,
    source_hash: u64,
) -> Option<(egui::TextureHandle, egui::Vec2)> {
    let width = width.max(80.0);
    let width_px = (width * scale).ceil().max(1.0) as u32;

    // First pass: lay out the document to learn its height, and cache any
    // local images it references so the real render can show them.
    let mut measuring = PixbufContainer::new_with_scale(width_px, 600, scale);
    let mut images: HashMap<String, Vec<u8>> = HashMap::new();
    let mut content_height = 600.0_f32;
    for _ in 0..3 {
        {
            let mut doc = Document::from_html(html, &mut measuring, None, None).ok()?;
            let _ = doc.render(width);
            content_height = doc.height().max(1.0);
        }

        let pending = measuring.take_pending_images();
        if pending.is_empty() {
            break;
        }
        for (src, _) in pending {
            if images.contains_key(&src) {
                continue;
            }
            if let Some(bytes) = load_local_image(&src) {
                measuring.load_image_data(&src, &bytes);
                images.insert(src, bytes);
            }
        }
    }

    // Keep the backing texture within common GPU limits.
    let max_height = (16_384.0 / scale).floor().max(64.0);
    let height = content_height.min(max_height);
    let height_px = (height * scale).ceil().max(1.0) as u32;

    let mut container = PixbufContainer::new_with_scale(width_px, height_px, scale);
    for (src, bytes) in &images {
        container.load_image_data(src, bytes);
    }
    {
        let mut doc = Document::from_html(html, &mut container, None, None).ok()?;
        let _ = doc.render(width);
        doc.draw(
            DrawContext::default(),
            0.0,
            0.0,
            Some(Position {
                x: 0.0,
                y: 0.0,
                width,
                height,
            }),
        );
    }

    let size = [container.width() as usize, container.height() as usize];
    let image = egui::ColorImage::from_rgba_premultiplied(size, container.pixels());
    let texture = ctx.load_texture(
        format!("html_preview_{source_hash}_{width_px}_{scale}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    Some((texture, egui::vec2(width, height)))
}

/// Read a local `file://` image so litehtml can draw it.
fn load_local_image(src: &str) -> Option<Vec<u8>> {
    let path = src.strip_prefix("file://")?;
    #[cfg(target_os = "windows")]
    let path = path.strip_prefix('/').unwrap_or(path);
    std::fs::read(Path::new(path)).ok()
}
