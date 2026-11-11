use eframe::egui;
use litehtml::pixbuf::PixbufContainer;
use litehtml::{Document, DrawContext, Position};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Wait for the pointer to stop moving before doing the expensive litehtml
/// hit test. Parsing/laying out the document on every mouse move makes the
/// whole app feel laggy.
const HOVER_CHECK_DELAY: Duration = Duration::from_millis(120);

/// Rasterized HTML preview. Rendering is cached until the source, width,
/// display scale or hovered-link state changes.
pub struct HtmlPreview {
    texture: Option<egui::TextureHandle>,
    size: egui::Vec2,
    key: Option<Key>,
    hovered_link: bool,
    hover_pos: Option<egui::Pos2>,
    last_pointer: Option<egui::Pos2>,
    hover_check_due: Option<Instant>,
    /// Latest state that still needs a render.
    desired: Option<RenderJob>,
    worker: Option<RenderWorker>,
    cached_html: Option<(u64, Arc<str>)>,
    texture_seq: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    source_hash: u64,
    width: u32,
    height: u32,
    scale_milli: u32,
    hovered_link: bool,
    image_version: u64,
}

struct RenderJob {
    key: Key,
    html: Arc<str>,
    width: f32,
    min_height: f32,
    scale: f32,
    hover_pos: Option<egui::Pos2>,
    height_hint: f32,
}

struct RenderOutcome {
    key: Key,
    image: Option<egui::ColorImage>,
    size: egui::Vec2,
}

struct RenderWorker {
    jobs: mpsc::Sender<RenderJob>,
    results: mpsc::Receiver<RenderOutcome>,
    pending: usize,
}

impl HtmlPreview {
    pub fn new() -> Self {
        Self {
            texture: None,
            size: egui::Vec2::ZERO,
            key: None,
            hovered_link: false,
            hover_pos: None,
            last_pointer: None,
            hover_check_due: None,
            desired: None,
            worker: None,
            cached_html: None,
            texture_seq: 0,
        }
    }

    fn ensure_worker(&mut self, ctx: &egui::Context) {
        if self.worker.is_some() {
            return;
        }
        let (job_tx, job_rx) = mpsc::channel::<RenderJob>();
        let (result_tx, result_rx) = mpsc::channel::<RenderOutcome>();
        let worker_ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(job) = job_rx.recv() {
                let rendered = render_image(
                    &worker_ctx,
                    &job.html,
                    job.width,
                    job.min_height,
                    job.scale,
                    job.hover_pos,
                    Some(job.height_hint),
                );
                let outcome = match rendered {
                    Some((image, size)) => RenderOutcome {
                        key: job.key,
                        image: Some(image),
                        size,
                    },
                    None => RenderOutcome {
                        key: job.key,
                        image: None,
                        size: egui::Vec2::ZERO,
                    },
                };
                if result_tx.send(outcome).is_err() {
                    break;
                }
                worker_ctx.request_repaint();
            }
        });
        self.worker = Some(RenderWorker {
            jobs: job_tx,
            results: result_rx,
            pending: 0,
        });
    }

    fn html_arc(&mut self, html: &str, source_hash: u64) -> Arc<str> {
        if let Some((hash, cached)) = &self.cached_html
            && *hash == source_hash
        {
            return Arc::clone(cached);
        }
        let arc: Arc<str> = Arc::from(html);
        self.cached_html = Some((source_hash, Arc::clone(&arc)));
        arc
    }

    fn install(&mut self, ctx: &egui::Context, outcome: RenderOutcome) {
        if let Some(image) = outcome.image {
            self.texture_seq = self.texture_seq.wrapping_add(1);
            self.texture = Some(ctx.load_texture(
                format!(
                    "html_preview_{}_{}_{}_{}",
                    outcome.key.source_hash,
                    outcome.key.width,
                    outcome.key.scale_milli,
                    self.texture_seq
                ),
                image,
                egui::TextureOptions::LINEAR,
            ));
            self.size = outcome.size;
        }
        self.key = Some(outcome.key);
    }

    fn render_sync(
        &mut self,
        ctx: &egui::Context,
        html: &str,
        key: Key,
        width: f32,
        min_height: f32,
        scale: f32,
    ) {
        if let Some((image, size)) =
            render_image(ctx, html, width, min_height, scale, self.hover_pos, None)
        {
            self.install(
                ctx,
                RenderOutcome {
                    key,
                    image: Some(image),
                    size,
                },
            );
        } else {
            self.key = Some(key);
        }
    }

    fn dispatch_latest(&mut self) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        // Only one render at a time; the newest request is kept in `desired`
        // and sent as soon as the current one finishes.
        if worker.pending > 0 {
            return;
        }
        let Some(job) = self.desired.take() else {
            return;
        };
        if self.key == Some(job.key) {
            return;
        }
        if worker.jobs.send(job).is_ok() {
            worker.pending = 1;
        }
    }

    fn settled(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|worker| worker.pending == 0)
            && self.desired.is_none()
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        html: &str,
        source_hash: u64,
        width: f32,
        min_height: f32,
    ) {
        let scale = ui.ctx().pixels_per_point();
        let key = Key {
            source_hash,
            width: width.round().max(1.0) as u32,
            height: min_height.round().max(1.0) as u32,
            scale_milli: (scale * 1000.0).round() as u32,
            hovered_link: self.hovered_link,
            image_version: remote_image_version(),
        };
        self.ensure_worker(ui.ctx());

        // Collect finished background renders. Only the newest one matters.
        let mut outcomes = Vec::new();
        if let Some(worker) = &mut self.worker {
            while let Ok(outcome) = worker.results.try_recv() {
                worker.pending = worker.pending.saturating_sub(1);
                outcomes.push(outcome);
            }
        }
        for outcome in outcomes {
            if outcome.key.source_hash == source_hash {
                self.install(ui.ctx(), outcome);
            }
        }

        let source_changed = self
            .key
            .is_none_or(|previous| previous.source_hash != source_hash);
        if self.texture.is_none() || source_changed {
            // A different document (or the first one) must appear immediately.
            self.desired = None;
            self.render_sync(ui.ctx(), html, key, width, min_height, scale);
        } else if self.key != Some(key) {
            // Width, hover or DPI changed: re-render in the background and keep
            // scaling the current texture so the panel tracks the drag now.
            let hint = if self.size.x > 0.0 {
                (self.size.y * (width / self.size.x).clamp(0.3, 2.5) * 1.08 + 24.0).max(min_height)
            } else {
                min_height
            };
            self.desired = Some(RenderJob {
                key,
                html: self.html_arc(html, source_hash),
                width,
                min_height,
                scale,
                hover_pos: if self.hovered_link {
                    self.hover_pos
                } else {
                    None
                },
                height_hint: hint,
            });
        } else {
            self.desired = None;
        }
        self.dispatch_latest();

        let Some(texture) = &self.texture else {
            ui.weak("Could not render the HTML preview.");
            return;
        };

        // Stretch the current texture to the live panel width while a crisp
        // render is in flight, so the content follows the drag frame by frame.
        let draw_w = width.max(1.0);
        let draw_scale = (draw_w / self.size.x.max(1.0)).clamp(0.05, 20.0);
        let draw_size = egui::vec2(draw_w, self.size.y * draw_scale);
        let response = ui.add(
            egui::Image::new(texture)
                .fit_to_exact_size(draw_size)
                .sense(egui::Sense::click()),
        );

        let pointer = response
            .hover_pos()
            .map(|p| (p - response.rect.min) / draw_scale.max(0.0001));
        if let Some(local) = pointer {
            if self.settled() {
                let pos = egui::pos2(local.x, local.y);
                let moved = self
                    .last_pointer
                    .is_none_or(|prev| (prev - pos).length() > 1.5);
                if moved {
                    self.last_pointer = Some(pos);
                    self.hover_check_due = Some(Instant::now() + HOVER_CHECK_DELAY);
                    ui.ctx().request_repaint_after(HOVER_CHECK_DELAY);
                } else if self
                    .hover_check_due
                    .is_some_and(|due| Instant::now() >= due)
                {
                    self.hover_check_due = None;
                    let is_link =
                        link_at(html, local.x, local.y, width, self.size.y, scale).is_some();
                    if is_link != self.hovered_link {
                        self.hovered_link = is_link;
                        self.hover_pos = is_link.then_some(pos);
                        // The changed key re-renders with the CSS :hover state
                        // in the background; keep the current texture until then.
                        ui.ctx().request_repaint();
                    }
                }
            }
            if self.hovered_link {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        } else {
            self.last_pointer = None;
            self.hover_check_due = None;
            if self.hovered_link {
                self.hovered_link = false;
                self.hover_pos = None;
                ui.ctx().request_repaint();
            }
        }

        if response.clicked()
            && self.settled()
            && let Some(local) = pointer
            && let Some(url) = link_at(html, local.x, local.y, width, self.size.y, scale)
        {
            open_link(&url);
        }
    }
}

fn open_link(url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:") {
        let _ = webbrowser::open(url);
    }
}

/// Hit-test `(x, y)` in logical document coordinates. Returns the clicked
/// anchor URL, if any.
fn link_at(html: &str, x: f32, y: f32, width: f32, height: f32, scale: f32) -> Option<String> {
    let width_u32 = width.ceil().max(1.0) as u32;
    let height_u32 = height.ceil().max(1.0) as u32;
    let mut container = PixbufContainer::new_with_scale(width_u32, height_u32, scale);
    let mut doc = Document::from_html(html, &mut container, None, None).ok()?;
    let _ = doc.render(width);
    doc.on_mouse_over(x, y, x, y);
    doc.on_lbutton_down(x, y, x, y);
    doc.on_lbutton_up(x, y, x, y);
    drop(doc);
    container.take_anchor_click()
}

/// Render `html` at `width` logical points into pixels. Returns the RGBA image
/// and its logical display size.
///
/// `height_hint` is the height of the previous render scaled to this width. It
/// lets the renderer allocate the pixmap before layout and skip the measuring
/// pass; the pixmap is grown once if the content turns out taller.
fn render_image(
    ctx: &egui::Context,
    html: &str,
    width: f32,
    min_height: f32,
    scale: f32,
    hover_pos: Option<egui::Pos2>,
    height_hint: Option<f32>,
) -> Option<(egui::ColorImage, egui::Vec2)> {
    let width = width.max(80.0);
    let width_u32 = width.ceil().max(1.0) as u32;

    // Keep the backing texture within common GPU limits.
    let max_height = (16_384.0 / scale).floor().max(64.0);

    // Load every image the page references up front so a single layout can
    // draw them. Remote images that are not cached yet are fetched in the
    // background and trigger a re-render when they arrive.
    let mut images: HashMap<String, Vec<u8>> = HashMap::new();
    for src in image_sources(html) {
        if let Some(bytes) = load_image_bytes(&src) {
            images.insert(src, bytes);
        } else {
            request_remote_image(ctx, &src);
        }
    }

    // Without a previous height, measure once so the pixmap is allocated
    // exactly; resizes reuse the last height and need only one layout.
    let guessed = if let Some(hint) = height_hint {
        hint.max(min_height).clamp(1.0, max_height)
    } else {
        let mut measuring = PixbufContainer::new_with_scale(width_u32, 600, scale);
        for (src, bytes) in &images {
            measuring.load_image_data(src, bytes);
        }
        let mut doc = Document::from_html(html, &mut measuring, None, None).ok()?;
        let _ = doc.render(width);
        doc.height().max(1.0).max(min_height).min(max_height)
    };

    let mut container_u32 = guessed.ceil().max(1.0) as u32;
    let mut container = PixbufContainer::new_with_scale(width_u32, container_u32, scale);
    for (src, bytes) in &images {
        container.load_image_data(src, bytes);
    }

    let mut doc = Document::from_html(html, &mut container, None, None).ok()?;
    let _ = doc.render(width);
    let content_height = doc.height().max(1.0);
    let wanted = content_height.max(min_height).min(max_height);
    let allocated_phys = (container_u32 as f32 * scale).ceil();

    // Grow once if the guess was too short; drawing is clipped by the pixmap.
    if (wanted * scale).ceil() > allocated_phys {
        drop(doc);
        container_u32 = (wanted * scale).ceil() as u32 + 1;
        container = PixbufContainer::new_with_scale(width_u32, container_u32, scale);
        for (src, bytes) in &images {
            container.load_image_data(src, bytes);
        }
        doc = Document::from_html(html, &mut container, None, None).ok()?;
        let _ = doc.render(width);
    }

    if let Some(pos) = hover_pos {
        doc.on_mouse_over(pos.x, pos.y, pos.x, pos.y);
        let _ = doc.render(width);
    }
    doc.draw(
        DrawContext::default(),
        0.0,
        0.0,
        Some(Position {
            x: 0.0,
            y: 0.0,
            width,
            height: wanted,
        }),
    );
    drop(doc);

    let pixel_width = container.width() as usize;
    let used_height = ((wanted * scale).ceil() as usize).min(container.height() as usize);
    let pixels = container.pixels();
    let needed = (pixel_width * used_height * 4).min(pixels.len());
    let image =
        egui::ColorImage::from_rgba_premultiplied([pixel_width, used_height], &pixels[..needed]);
    let display_height = used_height as f32 / scale.max(0.0001);
    Some((image, egui::vec2(width, display_height)))
}

/// Collect `src="..."` and `url(...)` values used by the page.
fn image_sources(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut sources = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find("src=") {
        let at = cursor + relative + 4;
        if let Some((value, end)) = quoted_value(html, at) {
            push_source(&mut sources, value);
            cursor = end;
        } else {
            cursor = at;
        }
    }
    cursor = 0;
    while let Some(relative) = lower[cursor..].find("url(") {
        let at = cursor + relative + 4;
        let rest = html[at..].trim_start();
        let value = if let Some(quoted) = rest.strip_prefix(['"', '\'']) {
            let quote = rest.chars().next().unwrap_or('"');
            match quoted.find(quote) {
                Some(end) => &quoted[..end],
                None => "",
            }
        } else {
            rest.split(')').next().unwrap_or("").trim()
        };
        push_source(&mut sources, value);
        cursor = at;
    }
    sources
}

fn push_source(sources: &mut Vec<String>, value: &str) {
    // Attribute values are decoded by the HTML parser before litehtml sees
    // them, so `&amp;` in the sidecar must be normalized here too or the
    // loaded image is cached under a key the renderer never asks for.
    let value = value
        .trim()
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&#x26;", "&");
    if !value.is_empty() && !sources.contains(&value) {
        sources.push(value);
    }
}

fn quoted_value(html: &str, at: usize) -> Option<(&str, usize)> {
    let rest = html[at..].trim_start();
    let leading = html[at..].len() - rest.len();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    Some((&inner[..end], at + leading + 1 + end + 1))
}

/// Remote images are downloaded in the background and cached in memory. Only a
/// small allowlist of image hosts is fetched, so generated HTML can't make the
/// app contact arbitrary servers.
static REMOTE_IMAGES: OnceLock<Mutex<HashMap<String, Arc<Vec<u8>>>>> = OnceLock::new();
static REMOTE_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static REMOTE_VERSION: AtomicU64 = AtomicU64::new(0);

fn remote_images() -> &'static Mutex<HashMap<String, Arc<Vec<u8>>>> {
    REMOTE_IMAGES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remote_inflight() -> &'static Mutex<HashSet<String>> {
    REMOTE_INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn remote_image_version() -> u64 {
    REMOTE_VERSION.load(Ordering::Relaxed)
}

fn cached_remote_image(src: &str) -> Option<Arc<Vec<u8>>> {
    remote_images().lock().ok()?.get(src).cloned()
}

fn allowed_image_url(src: &str) -> bool {
    let Some(rest) = src.strip_prefix("https://") else {
        return false;
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        host.as_str(),
        "loremflickr.com"
            | "images.unsplash.com"
            | "images.pexels.com"
            | "upload.wikimedia.org"
            | "thumb.wikimedia.org"
    )
}

fn request_remote_image(ctx: &egui::Context, src: &str) {
    if !allowed_image_url(src) || cached_remote_image(src).is_some() {
        return;
    }
    let Ok(mut inflight) = remote_inflight().lock() else {
        return;
    };
    if !inflight.insert(src.to_string()) {
        return;
    }
    drop(inflight);

    let ctx = ctx.clone();
    let url = src.to_string();
    std::thread::spawn(move || {
        if let Some(bytes) = fetch_remote_image(&url) {
            if let Ok(mut cache) = remote_images().lock() {
                cache.insert(url.clone(), Arc::new(bytes));
            }
            REMOTE_VERSION.fetch_add(1, Ordering::Relaxed);
            ctx.request_repaint();
        }
        if let Ok(mut inflight) = remote_inflight().lock() {
            inflight.remove(&url);
        }
    });
}

fn fetch_remote_image(url: &str) -> Option<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .ok()?;
    let resp = client
        .get(url)
        .header("User-Agent", "RustyNotepad/0.3")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if resp.content_length().unwrap_or(0) > 8_000_000 {
        return None;
    }
    let bytes = resp.bytes().ok()?;
    (!bytes.is_empty() && bytes.len() <= 8_000_000).then(|| bytes.to_vec())
}

/// Read a local `file://`, `data:`, or cached remote image so litehtml can draw
/// it. Remote images are fetched in the background by [`request_remote_image`].
fn load_image_bytes(src: &str) -> Option<Vec<u8>> {
    if src.starts_with("https://") || src.starts_with("http://") {
        return cached_remote_image(src).map(|bytes| bytes.as_ref().clone());
    }
    if let Some(encoded) = src.strip_prefix("data:") {
        let (_, data) = encoded.split_once(',')?;
        if encoded.contains(";base64,") {
            use base64::Engine as _;
            return base64::engine::general_purpose::STANDARD.decode(data).ok();
        }
        return Some(data.as_bytes().to_vec());
    }

    let path = src.strip_prefix("file://")?;
    #[cfg(target_os = "windows")]
    let path = path.strip_prefix('/').unwrap_or(path);
    std::fs::read(Path::new(path)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_decodes_image_sources() {
        let html = "<div style=\"background-image:url('https://thumb.wikimedia.org/a.jpg?x=1&amp;y=2')\"></div>\
                    <img src='file:///C:/t/a.jpg'><img src=\"data:image/png;base64,AA==\">";
        let sources = image_sources(html);
        assert!(
            sources
                .iter()
                .any(|source| source == "https://thumb.wikimedia.org/a.jpg?x=1&y=2"),
            "{sources:?}"
        );
        assert!(
            sources.iter().any(|source| source == "file:///C:/t/a.jpg"),
            "{sources:?}"
        );
        assert_eq!(sources.len(), 3, "{sources:?}");
    }

    #[test]
    fn quoted_values_end_after_the_closing_quote() {
        let html = "src=  \"a.jpg\" tail";
        let (value, end) = quoted_value(html, 4).unwrap();
        assert_eq!(value, "a.jpg");
        assert_eq!(&html[end..], " tail");
    }
}
