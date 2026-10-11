use crate::app::NotepadApp;
use crate::app::theme::{
    MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH, PREVIEW_MIN_WIDTH, panel_frame, panel_header,
};
use eframe::egui;
use egui_commonmark::CommonMarkViewer;
use std::fs;
use std::path::Path;

/// Convert a filesystem path to a `file://` URI understood by
/// `egui_extras::FileLoader` (handles Windows `C:\` -> `file:///C:/...`).
fn path_to_file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with("file://") {
        s
    } else if s.starts_with('/') {
        if cfg!(target_os = "windows") {
            format!("file:///{s}")
        } else {
            format!("file://{s}")
        }
    } else if s.len() >= 2 && s.as_bytes()[1] == b':' {
        // Windows drive letter: C:/...
        format!("file:///{s}")
    } else {
        format!("file://{s}")
    }
}

fn is_absolute_uri_dest(dest: &str) -> bool {
    dest.contains("://") || dest.starts_with("data:") || dest.starts_with("mailto:")
}

/// Rewrite bare relative image destinations (`![](img/foo.png)`) to absolute
/// `file://` URIs based on the directory of the open markdown file.
///
/// Without this, `egui_extras::FileLoader` resolves relative paths against
/// the process working directory, so images next to the `.md` file fail to load.
fn resolve_image_uris(markdown: &str, base_dir: Option<&Path>) -> String {
    let Some(base) = base_dir else {
        return markdown.to_string();
    };
    let bytes = markdown.as_bytes();
    let mut out = String::with_capacity(markdown.len());
    let mut i = 0;
    while i < bytes.len() {
        // Look for `![`
        if bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Find `](`
            if let Some(close_bracket) = markdown[i..].find("](") {
                let dest_start = i + close_bracket + 2;
                // Find closing `)` (handle `<path with spaces>` form).
                let rest = &markdown[dest_start..];
                let dest_end_rel = if rest.starts_with('<') {
                    rest.find('>')
                        .and_then(|gt| rest[gt..].find(')').map(|p| gt + p))
                } else {
                    // Take care with titles: `![a](uri "title")` — closing paren is last `)`.
                    // Simple scan: find `)` that ends the group, respecting quotes.
                    let mut in_single = false;
                    let mut in_double = false;
                    let mut end: Option<usize> = None;
                    for (idx, ch) in rest.char_indices() {
                        match ch {
                            '\'' if !in_double => in_single = !in_single,
                            '"' if !in_single => in_double = !in_double,
                            ')' if !in_single && !in_double => {
                                end = Some(idx);
                                break;
                            }
                            _ => {}
                        }
                    }
                    end
                };
                if let Some(end_rel) = dest_end_rel {
                    let dest_end = dest_start + end_rel;
                    let label_and_paren = &markdown[i..dest_start]; // `![alt](`
                    let dest_group = &markdown[dest_start..dest_end]; // `uri` or `uri "title"`
                    // Split uri from optional title.
                    let (uri_part, title_part) = split_dest_title(dest_group);
                    let uri_trimmed = uri_part.trim();
                    // Strip `<...>` wrapper if present.
                    let bare = uri_trimmed
                        .strip_prefix('<')
                        .and_then(|s| s.strip_suffix('>'))
                        .unwrap_or(uri_trimmed);
                    if bare.is_empty() || is_absolute_uri_dest(bare) {
                        out.push_str(&markdown[i..=dest_end]);
                    } else {
                        let p = Path::new(bare);
                        let abs = if p.is_absolute() {
                            p.to_path_buf()
                        } else {
                            base.join(p)
                        };
                        let new_uri = path_to_file_uri(&abs);
                        out.push_str(label_and_paren);
                        out.push_str(&new_uri);
                        out.push_str(title_part);
                        out.push(')');
                    }
                    i = dest_end + 1;
                    continue;
                }
            }
        }
        // Copy one char (keep UTF-8 boundaries).
        let ch = markdown[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Split `uri` + optional `"title"` tail, preserving original whitespace/title.
fn split_dest_title(group: &str) -> (&str, &str) {
    let g = group.trim_end();
    // Title must be separated by whitespace and quoted.
    // Find first whitespace that precedes a quote.
    let mut in_angle = false;
    for (idx, ch) in g.char_indices() {
        if ch == '<' {
            in_angle = true;
        } else if ch == '>' {
            in_angle = false;
        }
        if !in_angle && ch.is_whitespace() {
            let rest = g[idx..].trim_start();
            if rest.starts_with('"') || rest.starts_with('\'') || rest.starts_with('(') {
                return (g[..idx].trim_end(), &group[idx..]);
            }
        }
    }
    (group, "")
}

impl NotepadApp {
    pub fn show_editor(&mut self, ctx: &egui::Context) {
        // Live preview is always enabled as a resizable right-hand panel.
        let avail_w = ctx.available_rect().width();
        let default_w = if avail_w > 0.0 { avail_w * 0.5 } else { 400.0 };
        // One shared frame keeps all panel headers aligned at the same height.
        let frame = panel_frame(ctx);
        if self.tabs.preview {
            // Cap the width so the panel can never be dragged wide enough
            // to hide the central editor entirely.
            let max_preview_w = (avail_w - MIN_EDITOR_WIDTH).max(MIN_SIDE_PANEL_WIDTH);
            egui::SidePanel::right("preview_panel")
                .resizable(true)
                .default_width(default_w)
                .min_width(PREVIEW_MIN_WIDTH)
                .max_width(max_preview_w)
                .frame(frame)
                .show(ctx, |ui| {
                    if panel_header(ui, "Preview", "Hide preview").clicked() {
                        self.tabs.preview = false;
                    }
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add_space(2.0);
                            let mut processed_content = String::new();
                            for line in self.content.lines() {
                                processed_content.push_str(line);
                                processed_content.push_str("  \n");
                            }

                            let base_dir = self.file_path.as_ref().and_then(|p| {
                                if p.is_dir() {
                                    Some(p.clone())
                                } else {
                                    p.parent().map(|d| d.to_path_buf())
                                }
                            });
                            let with_images =
                                resolve_image_uris(&processed_content, base_dir.as_deref());
                            let max_w = (ui.available_width() as usize).max(100);

                            CommonMarkViewer::new("md_viewer")
                                .max_image_width(Some(max_w))
                                .show(ui, &mut self.markdown_cache, &with_images);
                        });
                });
        }

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if !self.tabs.editor {
                ui.centered_and_justified(|ui| {
                    ui.weak("Editor hidden — enable the Editor tab above.");
                });
                return;
            }
            if panel_header(ui, "Editor", "Hide editor").clicked() {
                self.tabs.editor = false;
            }
            ui.separator();

            let visuals = ui.visuals_mut();
            visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
            visuals.selection.stroke = egui::Stroke::NONE;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let max_width = ui.available_width();

                    let response = egui::TextEdit::multiline(&mut self.content)
                        .id(egui::Id::new("editor_text"))
                        .hint_text("Write text here...")
                        .frame(false)
                        .desired_width(max_width)
                        .layouter(&mut |ui, string, wrap_width| {
                            let mut layout_job = egui::text::LayoutJob::default();
                            layout_job.append(
                                string,
                                0.0,
                                egui::text::TextFormat {
                                    color: ui.visuals().widgets.noninteractive.text_color(),
                                    line_height: Some(18.0),
                                    ..Default::default()
                                },
                            );
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts(|f| f.layout_job(layout_job))
                        })
                        .min_size(egui::vec2(max_width, ui.available_height()))
                        .show(ui);

                    if response.response.changed() {
                        if let Some(ref path) = self.file_path {
                            if fs::write(path, &self.content).is_ok() {
                                self.status_message = String::from("Autosaved changes.");
                            } else {
                                self.status_message = String::from("Autosave failed.");
                            }
                        } else {
                            self.status_message =
                                String::from("Unsaved changes (Save file to enable autosave).");
                        }
                    }
                });
        });
    }
}
