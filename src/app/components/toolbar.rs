use crate::app::NotepadApp;
use crate::app::theme::{ACCENT, ACCENT_HOVERED, ACCENT_TEXT, BORDER, SURFACE};
use eframe::egui;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui_phosphor::regular as icons;

const EDITOR_ID: &str = "editor_text";

fn editor_id() -> egui::Id {
    egui::Id::new(EDITOR_ID)
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn load_range(ctx: &egui::Context) -> Option<CCursorRange> {
    TextEditState::load(ctx, editor_id()).and_then(|s| s.cursor.char_range())
}

fn store_range(ctx: &egui::Context, range: Option<CCursorRange>) {
    let id = editor_id();
    if let Some(mut state) = TextEditState::load(ctx, id) {
        state.cursor.set_char_range(range);
        state.store(ctx, id);
    }
    ctx.memory_mut(|m| m.request_focus(id));
}

fn line_block_range(content: &str, start_b: usize, end_b: usize) -> (usize, usize) {
    let start_b = start_b.min(content.len());
    let end_b = end_b.min(content.len());
    let start = content[..start_b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = content[end_b..]
        .find('\n')
        .map(|i| end_b + i)
        .unwrap_or(content.len());
    (start, end)
}

fn strip_heading_marker(line: &str) -> &str {
    let trimmed = line.trim_start();
    let mut hashes = 0;
    for c in trimmed.chars() {
        if c == '#' {
            hashes += 1;
        } else {
            break;
        }
        if hashes > 6 {
            break;
        }
    }
    if hashes > 0 && trimmed[hashes..].starts_with(' ') {
        &trimmed[hashes + 1..]
    } else {
        line
    }
}

/// Uniform icon button for the markdown toolbar.
/// Fixed size + larger icon font keeps the row aligned and scannable.
fn toolbar_icon(ui: &mut egui::Ui, icon: &str, hover: &str) -> egui::Response {
    ui.add_sized(
        [32.0, 28.0],
        egui::Button::new(egui::RichText::new(icon).size(18.0)),
    )
    .on_hover_text(hover)
}

/// Blend two opaque colors.
fn mix_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

/// Accent-filled dictation button. `icon` is the glyph to show, or `None` for
/// an animated spinner. `pulse` gently animates the control while recording or
/// transcribing. The size stays fixed so the toolbar never jumps.
fn dictation_button(
    ui: &mut egui::Ui,
    icon: Option<&str>,
    interactive: bool,
    pulse: bool,
    hover: &str,
) -> egui::Response {
    let sense = if interactive {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 28.0), sense);
    let response = response.on_hover_text(hover);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, hover));

    // Smooth 0..1 pulse while a recording or transcription is in progress.
    let wave = if pulse {
        let time = ui.input(|i| i.time) as f32;
        0.5 - 0.5 * (time * 3.2).cos()
    } else {
        0.0
    };

    let fill = if !interactive && !pulse {
        ACCENT.gamma_multiply(0.4)
    } else if response.hovered() && interactive {
        ACCENT_HOVERED
    } else if pulse {
        mix_color(ACCENT, ACCENT_HOVERED, 0.1 + 0.35 * wave)
    } else {
        ACCENT
    };

    if pulse {
        let ring = rect.expand(1.0 + 2.5 * wave);
        let ring_color = egui::Color32::from_rgba_unmultiplied(
            ACCENT_HOVERED.r(),
            ACCENT_HOVERED.g(),
            ACCENT_HOVERED.b(),
            (110.0 * (1.0 - wave)) as u8,
        );
        ui.painter().rect_stroke(
            ring,
            6.0 + 2.5 * wave,
            egui::Stroke::new(1.0_f32, ring_color),
        );
        ui.ctx().request_repaint();
    }

    ui.painter().rect_filled(rect, 5.0, fill);

    match icon {
        Some(icon) => {
            let color = if interactive || pulse {
                ACCENT_TEXT
            } else {
                ACCENT_TEXT.gamma_multiply(0.55)
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(18.0),
                color,
            );
        }
        None => {
            let spinner_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
            ui.put(spinner_rect, egui::Spinner::new().color(ACCENT_TEXT));
        }
    }

    if interactive {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

impl NotepadApp {
    fn selection_bytes(&self, ctx: &egui::Context) -> Option<(usize, usize)> {
        let range = load_range(ctx)?;
        let (mut a, mut b) = (range.primary.index, range.secondary.index);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        Some((
            char_to_byte(&self.content, a),
            char_to_byte(&self.content, b),
        ))
    }

    fn cursor_char_index(&self, ctx: &egui::Context) -> Option<usize> {
        let range = load_range(ctx)?;
        Some(range.primary.index.max(range.secondary.index))
    }

    /// Wrap the current selection with `before`/`after`, or insert
    /// `before + placeholder + after` at the cursor when nothing is selected.
    /// Toggles off when the selection is already wrapped.
    fn wrap_selection(
        &mut self,
        ctx: &egui::Context,
        before: &str,
        after: &str,
        placeholder: &str,
    ) {
        if let Some((start_b, end_b)) = self.selection_bytes(ctx) {
            if start_b != end_b {
                let selected = self.content[start_b..end_b].to_string();
                if selected.starts_with(before)
                    && selected.ends_with(after)
                    && selected.len() >= before.len() + after.len()
                {
                    let inner = selected[before.len()..selected.len() - after.len()].to_string();
                    self.content.replace_range(start_b..end_b, &inner);
                    let start_c = self.content[..start_b].chars().count();
                    let end_c = start_c + inner.chars().count();
                    store_range(
                        ctx,
                        Some(CCursorRange::two(
                            CCursor::new(start_c),
                            CCursor::new(end_c),
                        )),
                    );
                } else {
                    let new_text = format!("{before}{selected}{after}");
                    self.content.replace_range(start_b..end_b, &new_text);
                    let start_c = self.content[..start_b].chars().count();
                    let end_c = start_c + new_text.chars().count();
                    store_range(
                        ctx,
                        Some(CCursorRange::two(
                            CCursor::new(start_c),
                            CCursor::new(end_c),
                        )),
                    );
                }
            } else {
                let insert = format!("{before}{placeholder}{after}");
                self.content.insert_str(start_b, &insert);
                let cursor_c = self.content[..start_b].chars().count();
                let sel_start = cursor_c + before.chars().count();
                let sel_end = sel_start + placeholder.chars().count();
                store_range(
                    ctx,
                    Some(CCursorRange::two(
                        CCursor::new(sel_start),
                        CCursor::new(sel_end),
                    )),
                );
            }
        } else {
            if !self.content.is_empty() && !self.content.ends_with('\n') {
                self.content.push('\n');
            }
            self.content
                .push_str(&format!("{before}{placeholder}{after}"));
            let total_c = self.content.chars().count();
            let sel_end = total_c - after.chars().count();
            let sel_start = sel_end - placeholder.chars().count();
            store_range(
                ctx,
                Some(CCursorRange::two(
                    CCursor::new(sel_start),
                    CCursor::new(sel_end),
                )),
            );
        }
        self.note_content_changed();
    }

    /// Toggle a line prefix (quote, bullet, task) on every line in the
    /// current selection, or insert it on the current line when collapsed.
    fn toggle_line_prefix(&mut self, ctx: &egui::Context, prefix: &str, placeholder: &str) {
        let (start_b, end_b) = self
            .selection_bytes(ctx)
            .map(|(s, e)| line_block_range(&self.content, s, e))
            .unwrap_or_else(|| {
                let end = self.content.len();
                (end, end)
            });

        if start_b == end_b && start_b == self.content.len() {
            if !self.content.is_empty() && !self.content.ends_with('\n') {
                self.content.push('\n');
            }
            self.content.push_str(prefix);
            self.content.push_str(placeholder);
            let total_c = self.content.chars().count();
            store_range(
                ctx,
                Some(CCursorRange::two(
                    CCursor::new(total_c),
                    CCursor::new(total_c),
                )),
            );
            self.note_content_changed();
            return;
        }

        let block = self.content[start_b..end_b].to_string();
        let mut lines: Vec<String> = block.split('\n').map(|s| s.to_string()).collect();
        let all_prefixed = lines.iter().all(|l| l.trim_start().starts_with(prefix));
        for line in &mut lines {
            if all_prefixed {
                if let Some(pos) = line.find(prefix) {
                    line.replace_range(pos..pos + prefix.len(), "");
                }
            } else if !line.trim_start().starts_with(prefix) {
                line.insert_str(0, prefix);
            }
        }
        let new_block = lines.join("\n");
        self.content.replace_range(start_b..end_b, &new_block);
        let end_c = self.content[..start_b + new_block.len()].chars().count();
        store_range(
            ctx,
            Some(CCursorRange::two(CCursor::new(end_c), CCursor::new(end_c))),
        );
        self.note_content_changed();
    }

    fn apply_heading(&mut self, ctx: &egui::Context, level: u8) {
        let prefix = format!("{} ", "#".repeat(level as usize));
        let (start_b, end_b) = self
            .selection_bytes(ctx)
            .map(|(s, e)| line_block_range(&self.content, s, e))
            .unwrap_or_else(|| {
                let end = self.content.len();
                (end, end)
            });

        if start_b == end_b && start_b == self.content.len() {
            if !self.content.is_empty() && !self.content.ends_with('\n') {
                self.content.push('\n');
            }
            self.content.push_str(&prefix);
            self.content.push_str("Heading");
            let total_c = self.content.chars().count();
            store_range(
                ctx,
                Some(CCursorRange::two(
                    CCursor::new(total_c),
                    CCursor::new(total_c),
                )),
            );
            self.note_content_changed();
            return;
        }

        let block = self.content[start_b..end_b].to_string();
        let lines: Vec<String> = block.split('\n').map(|s| s.to_string()).collect();
        let mut out = Vec::with_capacity(lines.len());
        for line in lines {
            let stripped = strip_heading_marker(&line).to_string();
            // Toggle off if the line already has exactly this level.
            if line.trim_start().starts_with(&prefix) {
                out.push(stripped);
            } else {
                out.push(format!("{prefix}{stripped}"));
            }
        }
        let new_block = out.join("\n");
        self.content.replace_range(start_b..end_b, &new_block);
        let end_c = self.content[..start_b + new_block.len()].chars().count();
        store_range(
            ctx,
            Some(CCursorRange::two(CCursor::new(end_c), CCursor::new(end_c))),
        );
        self.note_content_changed();
    }

    fn apply_ordered_list(&mut self, ctx: &egui::Context) {
        let (start_b, end_b) = self
            .selection_bytes(ctx)
            .map(|(s, e)| line_block_range(&self.content, s, e))
            .unwrap_or_else(|| {
                let end = self.content.len();
                (end, end)
            });

        if start_b == end_b && start_b == self.content.len() {
            if !self.content.is_empty() && !self.content.ends_with('\n') {
                self.content.push('\n');
            }
            self.content.push_str("1. List item");
            let total_c = self.content.chars().count();
            store_range(
                ctx,
                Some(CCursorRange::two(
                    CCursor::new(total_c),
                    CCursor::new(total_c),
                )),
            );
            self.note_content_changed();
            return;
        }

        let block = self.content[start_b..end_b].to_string();
        let lines: Vec<String> = block.split('\n').map(|s| s.to_string()).collect();
        let numbered = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let t = l.trim_start();
                // Strip an existing "N. " marker if present.
                let mut rest = l.as_str();
                if let Some(dot) = t.find(". ") {
                    let (num, _) = t.split_at(dot);
                    if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
                        let offset = l.len() - t.len();
                        rest = &l[offset + dot + 2..];
                    }
                }
                format!("{}. {rest}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.content.replace_range(start_b..end_b, &numbered);
        let end_c = self.content[..start_b + numbered.len()].chars().count();
        store_range(
            ctx,
            Some(CCursorRange::two(CCursor::new(end_c), CCursor::new(end_c))),
        );
        self.note_content_changed();
    }

    /// Insert a (possibly multi-line) snippet at the cursor, ensuring blank
    /// lines around block-level constructs.
    fn insert_block(&mut self, ctx: &egui::Context, snippet: &str) {
        if let Some(idx) = self.cursor_char_index(ctx) {
            let byte_pos = char_to_byte(&self.content, idx);
            let mut to_insert = String::new();
            if byte_pos > 0 && !self.content[..byte_pos].ends_with("\n\n") {
                if self.content[..byte_pos].ends_with('\n') {
                    to_insert.push('\n');
                } else {
                    to_insert.push_str("\n\n");
                }
            }
            to_insert.push_str(snippet);
            to_insert.push_str("\n\n");
            self.content.insert_str(byte_pos, &to_insert);
            let new_c = self.content[..byte_pos + to_insert.len()].chars().count();
            store_range(
                ctx,
                Some(CCursorRange::two(CCursor::new(new_c), CCursor::new(new_c))),
            );
        } else {
            if !self.content.is_empty() && !self.content.ends_with('\n') {
                self.content.push('\n');
            }
            if !self.content.is_empty() {
                self.content.push('\n');
            }
            self.content.push_str(snippet);
            self.content.push('\n');
        }
        self.note_content_changed();
    }

    pub fn show_markdown_toolbar(&mut self, ctx: &egui::Context) {
        // Dispatch id set by the clicked button below.
        let mut action: Option<u8> = None;
        let dictating = self.is_dictating();

        egui::TopBottomPanel::top("markdown_toolbar")
            .frame(
                egui::Frame::none()
                    .fill(crate::app::theme::BG_ELEVATED)
                    .inner_margin(egui::Margin {
                        left: 8.0,
                        right: 8.0,
                        top: 5.0,
                        bottom: 6.0,
                    }),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);

                    // Inline formatting
                    if toolbar_icon(ui, icons::TEXT_B, "Bold (**text**)").clicked() {
                        action = Some(0);
                    }
                    if toolbar_icon(ui, icons::TEXT_ITALIC, "Italic (*text*)").clicked() {
                        action = Some(1);
                    }
                    if toolbar_icon(ui, icons::TEXT_STRIKETHROUGH, "Strikethrough (~~text~~)")
                        .clicked()
                    {
                        action = Some(2);
                    }
                    if toolbar_icon(ui, icons::CODE_SIMPLE, "Inline code (`code`)").clicked() {
                        action = Some(3);
                    }
                    ui.separator();
                    // Headings
                    if toolbar_icon(ui, icons::TEXT_H_ONE, "Heading 1 (# )").clicked() {
                        action = Some(4);
                    }
                    if toolbar_icon(ui, icons::TEXT_H_TWO, "Heading 2 (## )").clicked() {
                        action = Some(5);
                    }
                    if toolbar_icon(ui, icons::TEXT_H_THREE, "Heading 3 (### )").clicked() {
                        action = Some(6);
                    }
                    ui.separator();
                    // Blocks / lists
                    if toolbar_icon(ui, icons::QUOTES, "Blockquote (> )").clicked() {
                        action = Some(7);
                    }
                    if toolbar_icon(ui, icons::LIST_BULLETS, "Bullet list (- )").clicked() {
                        action = Some(8);
                    }
                    if toolbar_icon(ui, icons::LIST_NUMBERS, "Ordered list (1. 2. 3.)").clicked() {
                        action = Some(9);
                    }
                    if toolbar_icon(ui, icons::CHECK_SQUARE, "Task list (- [ ] )").clicked() {
                        action = Some(10);
                    }
                    ui.separator();
                    // Inserts
                    if toolbar_icon(ui, icons::LINK_SIMPLE, "Link ([text](url))").clicked() {
                        action = Some(11);
                    }
                    if toolbar_icon(ui, icons::IMAGE, "Image (![alt](url))").clicked() {
                        action = Some(12);
                    }
                    if toolbar_icon(ui, icons::CODE_BLOCK, "Code block (```)").clicked() {
                        action = Some(13);
                    }
                    if toolbar_icon(ui, icons::MINUS, "Horizontal rule (---)").clicked() {
                        action = Some(14);
                    }
                    if toolbar_icon(ui, icons::TABLE, "Insert table").clicked() {
                        action = Some(15);
                    }
                    ui.separator();
                    // Document dictation. One accent button changes icon with
                    // state; the timer/status sits in a fixed container to its
                    // right so the row stays centered and stable.
                    let show_spinner = self.transcribing || self.live_finishing;
                    if show_spinner {
                        let label = if self.transcribing {
                            "Transcribing…"
                        } else {
                            "Finishing…"
                        };
                        ui.allocate_ui_with_layout(
                            egui::vec2(138.0, 28.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                dictation_button(ui, None, false, true, "Finishing dictation…");
                                ui.add_space(10.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(96.0, 28.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        egui::Frame::none()
                                            .fill(SURFACE)
                                            .stroke(egui::Stroke::new(1.0_f32, BORDER))
                                            .rounding(6.0)
                                            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.label(egui::RichText::new(label).small());
                                            });
                                    },
                                );
                            },
                        );
                    } else if self.live.is_some() {
                        let secs = self
                            .live
                            .as_ref()
                            .map(|session| session.elapsed().as_secs())
                            .unwrap_or(0);
                        let stop = ui
                            .allocate_ui_with_layout(
                                egui::vec2(106.0, 28.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let stop = dictation_button(
                                        ui,
                                        Some(icons::RECORD),
                                        true,
                                        true,
                                        "Stop and finish dictation",
                                    )
                                    .clicked();
                                    ui.add_space(10.0);
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(64.0, 28.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            egui::Frame::none()
                                                .fill(SURFACE)
                                                .stroke(egui::Stroke::new(1.0_f32, BORDER))
                                                .rounding(6.0)
                                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                                .show(ui, |ui| {
                                                    ui.colored_label(
                                                        ACCENT,
                                                        egui::RichText::new(format!(
                                                            "{:02}:{:02}",
                                                            secs / 60,
                                                            secs % 60
                                                        ))
                                                        .strong(),
                                                    );
                                                });
                                        },
                                    );
                                    stop
                                },
                            )
                            .inner;
                        if stop {
                            self.stop_live_dictation();
                        }
                    } else {
                        let start = ui
                            .allocate_ui_with_layout(
                                egui::vec2(32.0, 28.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    dictation_button(
                                        ui,
                                        Some(icons::MICROPHONE),
                                        true,
                                        false,
                                        "Dictate into the document in real time",
                                    )
                                    .clicked()
                                },
                            )
                            .inner;
                        if start {
                            self.start_live_dictation(ctx);
                        }
                    }
                });
            });

        if dictating {
            // Formatting would move the streamed insertion point.
            return;
        }
        match action {
            Some(0) => self.wrap_selection(ctx, "**", "**", "bold text"),
            Some(1) => self.wrap_selection(ctx, "*", "*", "italic text"),
            Some(2) => self.wrap_selection(ctx, "~~", "~~", "struck text"),
            Some(3) => self.wrap_selection(ctx, "`", "`", "code"),
            Some(4) => self.apply_heading(ctx, 1),
            Some(5) => self.apply_heading(ctx, 2),
            Some(6) => self.apply_heading(ctx, 3),
            Some(7) => self.toggle_line_prefix(ctx, "> ", "Quote"),
            Some(8) => self.toggle_line_prefix(ctx, "- ", "List item"),
            Some(9) => self.apply_ordered_list(ctx),
            Some(10) => self.toggle_line_prefix(ctx, "- [ ] ", "Task"),
            Some(11) => self.wrap_selection(ctx, "[", "](https://example.com)", "link text"),
            Some(12) => {
                self.wrap_selection(ctx, "![", "](https://example.com/image.png)", "alt text")
            }
            Some(13) => self.insert_block(ctx, "```\ncode\n```"),
            Some(14) => self.insert_block(ctx, "---"),
            Some(15) => self.insert_block(
                ctx,
                "| Header 1 | Header 2 |\n| --- | --- |\n| Cell | Cell |",
            ),
            _ => {}
        }
    }
}
