use crate::app::NotepadApp;
use crate::app::templates;
use crate::app::theme::{
    ACCENT_LINK, BG_CODE, BORDER, MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH,
    PREVIEW_MAX_CONTENT_WIDTH, PREVIEW_MIN_WIDTH, panel_frame, panel_header,
};
use eframe::egui;
use egui_commonmark::CommonMarkViewer;
use egui_phosphor::regular as icons;
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

/// Typography for the rendered preview: slightly larger, roomier text with
/// neutral borders around code blocks and a comfortable link color.
fn preview_style(ui: &egui::Ui) -> egui::Style {
    let mut style = (**ui.style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.5));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(30.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.5));
    style.spacing.item_spacing = egui::vec2(6.0, 7.0);
    style.visuals.hyperlink_color = ACCENT_LINK;
    style.visuals.code_bg_color = BG_CODE;
    style.visuals.extreme_bg_color = BG_CODE;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
    style
}

impl NotepadApp {
    pub fn show_editor(&mut self, ctx: &egui::Context) {
        let show_editor = self.tabs.editor;
        let show_preview = self.tabs.preview;
        let avail_w = ctx.available_rect().width();
        let frame = panel_frame(ctx);

        if show_preview && show_editor {
            // Side-by-side: preview is a resizable right-hand panel capped so
            // the central editor always keeps a usable width.
            let default_w = if avail_w > 0.0 { avail_w * 0.5 } else { 400.0 };
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
                        .show(ui, |ui| self.show_preview_body(ui, false));
                });
            self.show_editor_central(ctx, frame);
        } else if show_preview {
            // Editor hidden: the preview takes the whole central area, with no
            // panel header, so it can be read (or shown fullscreen) edge to edge.
            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| self.show_preview_body(ui, true));
            });
        } else if show_editor {
            self.show_editor_central(ctx, frame);
        } else {
            self.show_empty_workspace(ctx, frame);
        }
    }

    fn show_editor_central(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
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
                                    line_height: Some(20.0),
                                    ..Default::default()
                                },
                            );
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts(|f| f.layout_job(layout_job))
                        })
                        .min_size(egui::vec2(max_width, ui.available_height()))
                        .show(ui);

                    if response.response.changed() {
                        self.note_content_changed();
                    }
                });
        });
    }

    fn show_preview_body(&mut self, ui: &mut egui::Ui, full_width: bool) {
        ui.set_style(preview_style(ui));

        // A centered reading column keeps long lines comfortable when the
        // preview fills a wide window; narrow side panels use all their width.
        let pad_x = if full_width { 28.0 } else { 16.0 };
        let pad_top = if full_width { 18.0 } else { 10.0 };
        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: pad_x,
                right: pad_x,
                top: pad_top,
                bottom: 28.0,
            })
            .show(ui, |ui| {
                if self.content.trim().is_empty() {
                    ui.add_space((ui.available_height() * 0.24).clamp(0.0, 140.0));
                    let mut chosen: Option<&'static templates::Template> = None;
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(icons::TEXT_AA)
                                .size(38.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Nothing to preview yet").strong().size(16.0));
                        ui.label(
                            egui::RichText::new(
                                "Start writing in the editor and the formatted document appears here.",
                            )
                            .weak(),
                        );

                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new("Or start from a template")
                                .weak()
                                .small(),
                        );
                        ui.add_space(5.0);

                        let available = ui.available_width();
                        let columns = if available >= 280.0 { 2 } else { 1 };
                        let gap = 8.0;
                        let button_w = if columns == 2 {
                            ((available.min(380.0) - gap) / 2.0).max(120.0)
                        } else {
                            available.min(240.0)
                        };
                        let block_w =
                            columns as f32 * button_w + (columns as f32 - 1.0) * gap;
                        ui.allocate_ui_with_layout(
                            egui::vec2(block_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::Grid::new("template_grid")
                                    .num_columns(columns)
                                    .spacing([gap, 6.0])
                                    .show(ui, |ui| {
                                        for (index, template) in
                                            templates::TEMPLATES.iter().enumerate()
                                        {
                                            let button = egui::Button::new(
                                                egui::RichText::new(format!(
                                                    "{} {}",
                                                    template.icon, template.name
                                                ))
                                                .size(13.0),
                                            );
                                            if ui
                                                .add_sized([button_w, 28.0], button)
                                                .on_hover_text(template.description)
                                                .clicked()
                                            {
                                                chosen = Some(template);
                                            }
                                            if (index + 1) % columns == 0 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                            },
                        );
                    });

                    if let Some(template) = chosen {
                        self.content = template.body.to_string();
                        self.note_content_changed();
                        ui.ctx()
                            .memory_mut(|m| m.request_focus(egui::Id::new("editor_text")));
                    }
                    return;
                }

                let full_w = ui.available_width();
                let content_w = full_w.min(PREVIEW_MAX_CONTENT_WIDTH);
                let side = ((full_w - content_w) * 0.5).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width(content_w);

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
                        let max_w = (content_w as usize).max(100);

                        CommonMarkViewer::new("md_viewer")
                            .max_image_width(Some(max_w))
                            .show(ui, &mut self.markdown_cache, &with_images);
                    });
                });
            });
    }

    fn show_empty_workspace(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(icons::TEXT_AA)
                            .size(40.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Both panels are hidden")
                            .strong()
                            .size(16.0),
                    );
                    ui.add_space(4.0);
                    if ui.button("Show editor").clicked() {
                        self.tabs.editor = true;
                    }
                    if ui.button("Show preview").clicked() {
                        self.tabs.preview = true;
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_image_becomes_file_uri() {
        let base = Path::new("/notes");
        let out = resolve_image_uris("![x](images/pic.png)", Some(base));
        #[cfg(target_os = "windows")]
        assert!(out.contains("file:///"));
        #[cfg(not(target_os = "windows"))]
        assert!(out.contains("file:///notes/images/pic.png"));
    }

    #[test]
    fn absolute_image_and_links_are_left_alone() {
        let base = Path::new("/notes");
        let src = "![a](https://example.com/x.png) [link](relative.md)";
        assert_eq!(resolve_image_uris(src, Some(base)), src);
    }

    #[test]
    fn image_title_is_preserved() {
        let out = resolve_image_uris("![x](pic.png \"hello\")", Some(Path::new("/n")));
        assert!(out.contains("pic.png"));
        assert!(out.contains("\"hello\""));
    }
}
