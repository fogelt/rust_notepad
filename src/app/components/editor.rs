use crate::app::NotepadApp;
use crate::app::config::{EditorFont, UiSettings};
use crate::app::templates;
use crate::app::theme::{
    ACCENT, ACCENT_HOVERED, ACCENT_LINK, ACCENT_TEXT, BG_CODE, BORDER, MIN_EDITOR_WIDTH,
    MIN_SIDE_PANEL_WIDTH, PREVIEW_MAX_CONTENT_WIDTH, PREVIEW_MIN_WIDTH, SURFACE, SURFACE_ACTIVE,
    SURFACE_HOVERED, panel_frame, panel_header,
};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui_commonmark::CommonMarkViewer;
use egui_phosphor::regular as icons;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
        let Some(ch) = markdown[i..].chars().next() else {
            break;
        };
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
fn preview_style(ui: &egui::Ui, settings: &UiSettings) -> egui::Style {
    let mut style = (**ui.style()).clone();
    let body = settings.preview_font_size;
    let family = match settings.preview_font {
        EditorFont::Sans => egui::FontFamily::Proportional,
        EditorFont::Monospace => egui::FontFamily::Monospace,
    };
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(body, family.clone()),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(body * 1.94, family.clone()),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(body * 0.77, family.clone()),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(body * 0.87),
    );
    style.spacing.item_spacing = egui::vec2(6.0, 7.0);
    style.visuals.hyperlink_color = ACCENT_LINK;
    style.visuals.code_bg_color = BG_CODE;
    style.visuals.extreme_bg_color = BG_CODE;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
    style
}

/// Shared label style for template and empty-state buttons: accent-colored icon
/// followed by normal button text.
fn icon_label(ui: &egui::Ui, icon: &str, label: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("{icon}  "),
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(13.0),
            color: ACCENT,
            ..Default::default()
        },
    );
    job.append(
        label,
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(13.0),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    job
}

/// Icon-only segmented tab that mirrors the top bar's Assistant/Editor/Preview
/// tabs. Used for the Visual/Text preview modes.
fn preview_tab(
    ui: &mut egui::Ui,
    selected: bool,
    icon: &str,
    hover: &str,
    rounding: egui::Rounding,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = response.hovered();
    let fill = if selected {
        if hovered { ACCENT_HOVERED } else { ACCENT }
    } else if hovered {
        SURFACE_ACTIVE
    } else {
        SURFACE_HOVERED
    };
    ui.painter().rect_filled(rect, rounding, fill);

    let color = if selected {
        ACCENT_TEXT
    } else if hovered {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        color,
    );

    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, selected, hover));
    response.on_hover_text(hover).clicked()
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
                    let mut visual_clicked = false;
                    let mut text_clicked = false;
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Preview").strong().size(14.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::app::theme::header_close_button(ui, "Hide preview").clicked()
                            {
                                self.tabs.preview = false;
                            }
                            ui.add_space(6.0);
                            egui::Frame::none()
                                .fill(SURFACE)
                                .rounding(6.0)
                                .inner_margin(egui::Margin::ZERO)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    // This row inherits the right-to-left
                                    // layout, so add Text first to place it on
                                    // the right and Visual on the left.
                                    ui.horizontal(|ui| {
                                        text_clicked = preview_tab(
                                            ui,
                                            !self.preview_html,
                                            icons::TEXT_AA,
                                            "Text preview",
                                            egui::Rounding {
                                                nw: 0.0,
                                                sw: 0.0,
                                                ne: 5.0,
                                                se: 5.0,
                                            },
                                        );
                                        visual_clicked = preview_tab(
                                            ui,
                                            self.preview_html,
                                            icons::CHART_BAR,
                                            "Visual report",
                                            egui::Rounding {
                                                nw: 5.0,
                                                sw: 5.0,
                                                ne: 0.0,
                                                se: 0.0,
                                            },
                                        );
                                    });
                                });
                        });
                    });
                    if visual_clicked {
                        self.preview_html = true;
                        self.preview_revision = self.preview_revision.wrapping_add(1);
                    }
                    if text_clicked {
                        self.preview_html = false;
                        self.preview_revision = self.preview_revision.wrapping_add(1);
                    }
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_source(("preview_scroll", self.preview_html, self.preview_revision))
                        .auto_shrink([false; 2])
                        .show(ui, |ui| self.show_preview_body(ui, false));
                });
            self.show_editor_central(ctx, frame);
        } else if show_preview {
            // Editor hidden: the preview takes the whole central area, with no
            // panel header, so it can be read (or shown fullscreen) edge to edge.
            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_source(("preview_scroll", self.preview_html, self.preview_revision))
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

            if self.is_dictating() {
                let (badge_icon, badge_label) = if self.dictation_stopping {
                    (icons::WAVEFORM, "Finishing")
                } else {
                    (icons::MICROPHONE, "Dictating")
                };
                egui::Frame::none()
                    .stroke(egui::Stroke::new(1.5_f32, ACCENT))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    egui::Frame::none()
                                        .fill(ACCENT)
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{badge_icon} {badge_label}"
                                                ))
                                                .size(11.0)
                                                .strong()
                                                .color(ACCENT_TEXT),
                                            );
                                        });
                                },
                            );
                        });
                        ui.add_space(4.0);
                        self.show_editor_text(ui, true);
                    });
            } else {
                self.show_editor_text(ui, false);
            }
        });
    }

    /// The scrollable text area. Read-only while dictation owns the document.
    fn show_editor_text(&mut self, ui: &mut egui::Ui, locked: bool) {
        let family = match self.ui_settings.editor_font {
            EditorFont::Sans => egui::FontFamily::Proportional,
            EditorFont::Monospace => egui::FontFamily::Monospace,
        };
        let font_size = self.ui_settings.editor_font_size;
        let line_height = self.ui_settings.editor_line_height;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let max_width = ui.available_width();

                let response = egui::TextEdit::multiline(&mut self.content)
                    .id(egui::Id::new("editor_text"))
                    .hint_text("Write text here...")
                    .frame(false)
                    .interactive(!locked)
                    .desired_width(max_width)
                    .layouter(&mut move |ui, string, wrap_width| {
                        let mut layout_job = egui::text::LayoutJob::default();
                        layout_job.append(
                            string,
                            0.0,
                            egui::text::TextFormat {
                                font_id: egui::FontId::new(font_size, family.clone()),
                                color: ui.visuals().widgets.noninteractive.text_color(),
                                line_height: Some(line_height),
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
    }

    fn show_preview_body(&mut self, ui: &mut egui::Ui, full_width: bool) {
        if self.preview_html {
            self.show_html_preview_body(ui);
            return;
        }

        ui.set_style(preview_style(ui, &self.ui_settings));

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
                    let logo = self.logo_texture(ui.ctx());
                    let mut chosen: Option<&'static templates::Template> = None;
                    ui.vertical_centered(|ui| {
                        ui.add(
                            egui::Image::new(&logo)
                                .fit_to_exact_size(egui::vec2(72.0, 72.0)),
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
                                .small()
                                .color(ACCENT_LINK),
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
                                    .striped(false)
                                    .show(ui, |ui| {
                                        for (index, template) in
                                            templates::TEMPLATES.iter().enumerate()
                                        {
                                            let label =
                                                icon_label(ui, template.icon, template.name);
                                            if ui
                                                .add_sized(
                                                    [button_w, 28.0],
                                                    egui::Button::new(label),
                                                )
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

                        let line_spacing = self.ui_settings.preview_line_factor();
                        CommonMarkViewer::new("md_viewer")
                            .max_image_width(Some(max_w))
                            .line_spacing(line_spacing)
                            .show(ui, &mut self.markdown_cache, &with_images);
                    });
                });
            });
    }

    /// Show the assistant-designed HTML, or a call-to-action when none exists.
    fn show_html_preview_body(&mut self, ui: &mut egui::Ui) {
        if self.styled_html.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(48.0);
                if self.html_busy {
                    ui.spinner();
                    ui.add_space(6.0);
                    ui.label("Creating your visual report…");
                    let secs = self
                        .html_started
                        .map(|started| started.elapsed().as_secs())
                        .unwrap_or(0);
                    ui.weak(if secs > 0 {
                        format!("{secs}s elapsed · large documents can take a minute or two.")
                    } else {
                        String::from("Large documents can take a minute or two.")
                    });
                    ui.add_space(10.0);
                    if ui.button("Cancel").clicked() {
                        self.html_rx = None;
                        self.html_busy = false;
                        self.html_started = None;
                        self.status_message = String::from("Visual report cancelled.");
                    }
                } else {
                    ui.label(egui::RichText::new("No visual report yet").strong());
                    ui.add_space(4.0);
                    ui.weak(
                        "Summarize this document as a visual report with charts and highlights.",
                    );
                    ui.add_space(12.0);
                    let enabled = !self.chat_busy && !self.content.trim().is_empty();
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(format!("{}  Create visual report", icons::SPARKLE)),
                        )
                        .clicked()
                    {
                        self.generate_styled_html();
                    }
                    if let Some(error) = &self.html_error {
                        ui.add_space(10.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(error)
                                    .color(ui.visuals().error_fg_color)
                                    .small(),
                            )
                            .wrap(true),
                        );
                    }
                }
            });
            return;
        }

        if self.html_busy {
            ui.horizontal(|ui| {
                ui.spinner();
                let secs = self
                    .html_started
                    .map(|started| started.elapsed().as_secs())
                    .unwrap_or(0);
                ui.label(format!("Updating visual report… {secs}s"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.html_rx = None;
                        self.html_busy = false;
                        self.html_started = None;
                        self.status_message = String::from("Visual report cancelled.");
                    }
                });
            });
            ui.separator();
        }

        let Some(html) = self.styled_html.as_deref() else {
            return;
        };
        let mut hasher = DefaultHasher::new();
        html.hash(&mut hasher);
        let source_hash = hasher.finish();
        let content_w = ui.available_width().max(160.0);
        let min_height = ui.available_height().max(1.0);

        if self.html_preview.is_none() {
            self.html_preview = Some(crate::app::components::html_preview::HtmlPreview::new());
        }
        if let Some(preview) = &mut self.html_preview {
            preview.show(ui, html, source_hash, content_w, min_height);
        }
    }

    /// Load the bundled app mark once and reuse the texture.
    fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if let Some(texture) = &self.logo_texture {
            return texture.clone();
        }
        let bytes = include_bytes!("../../../assets/icon.png");
        let color_image = match image::load_from_memory(bytes) {
            Ok(image) => {
                let image = image.to_rgba8();
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                )
            }
            // Bundled at compile time; fall back to a transparent pixel rather
            // than taking the app down if the asset is ever invalid.
            Err(_) => egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
        };
        let texture = ctx.load_texture(
            "rusty_notepad_logo",
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.logo_texture = Some(texture.clone());
        texture
    }

    fn show_empty_workspace(&mut self, ctx: &egui::Context, frame: egui::Frame) {
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Centre the group vertically as well as horizontally.
                ui.add_space(((ui.available_height() - 180.0) * 0.5).max(8.0));
                let logo = self.logo_texture(ctx);
                ui.add(egui::Image::new(&logo).fit_to_exact_size(egui::vec2(80.0, 80.0)));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Both panels are hidden")
                        .strong()
                        .size(16.0),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Choose a panel to bring back.")
                        .weak()
                        .small(),
                );
                ui.add_space(16.0);

                let show_editor =
                    egui::Button::new(icon_label(ui, icons::PENCIL_SIMPLE, "Show editor"));
                if ui.add_sized([170.0, 30.0], show_editor).clicked() {
                    self.tabs.editor = true;
                }

                ui.add_space(6.0);
                let show_preview = egui::Button::new(icon_label(ui, icons::EYE, "Show preview"));
                if ui.add_sized([170.0, 30.0], show_preview).clicked() {
                    self.tabs.preview = true;
                }
            });
        });
    }
}

/// Insert `snippet` at the editor cursor, or append it to the document when the
/// editor has no stored cursor.
pub(crate) fn insert_at_cursor(app: &mut NotepadApp, ctx: &egui::Context, snippet: &str) {
    let id = egui::Id::new("editor_text");
    if let Some(state) = TextEditState::load(ctx, id)
        && let Some(range) = state.cursor.char_range()
    {
        let idx = range.primary.index.max(range.secondary.index);
        let byte_pos = app
            .content
            .char_indices()
            .nth(idx)
            .map(|(b, _)| b)
            .unwrap_or(app.content.len());
        let mut to_insert = snippet.to_string();
        if !to_insert.ends_with('\n') {
            to_insert.push('\n');
        }
        app.content.insert_str(byte_pos, &to_insert);
        let new_c = app.content[..byte_pos + to_insert.len()].chars().count();
        if let Some(mut st) = TextEditState::load(ctx, id) {
            st.cursor
                .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(new_c))));
            st.store(ctx, id);
        }
        ctx.memory_mut(|m| m.request_focus(id));
        return;
    }
    if !app.content.is_empty() && !app.content.ends_with('\n') {
        app.content.push('\n');
    }
    app.content.push_str(snippet);
    if !app.content.ends_with('\n') {
        app.content.push('\n');
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
