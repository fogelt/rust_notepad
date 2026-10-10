use crate::app::NotepadApp;
use eframe::egui;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;

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
        self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
            self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
        self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
            self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
        self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
            self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
        self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
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
        self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
    }

    pub fn show_markdown_toolbar(&mut self, ctx: &egui::Context) {
        // Dispatch id set by the clicked button below.
        let mut action: Option<u8> = None;

        egui::TopBottomPanel::top("markdown_toolbar").show(ctx, |ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if ui
                            .button(egui::RichText::new("B").strong())
                            .on_hover_text("Bold (**text**)")
                            .clicked()
                        {
                            action = Some(0);
                        }
                        if ui
                            .button(egui::RichText::new("I").italics())
                            .on_hover_text("Italic (*text*)")
                            .clicked()
                        {
                            action = Some(1);
                        }
                        if ui
                            .button(egui::RichText::new("S").strikethrough())
                            .on_hover_text("Strikethrough (~~text~~)")
                            .clicked()
                        {
                            action = Some(2);
                        }
                        if ui
                            .button(egui::RichText::new("Code").code())
                            .on_hover_text("Inline code (`code`)")
                            .clicked()
                        {
                            action = Some(3);
                        }
                        ui.separator();
                        if ui.button("H1").on_hover_text("Heading 1 (# )").clicked() {
                            action = Some(4);
                        }
                        if ui.button("H2").on_hover_text("Heading 2 (## )").clicked() {
                            action = Some(5);
                        }
                        if ui.button("H3").on_hover_text("Heading 3 (### )").clicked() {
                            action = Some(6);
                        }
                        ui.separator();
                        if ui
                            .button("Quote")
                            .on_hover_text("Blockquote (> )")
                            .clicked()
                        {
                            action = Some(7);
                        }
                        if ui
                            .button("List")
                            .on_hover_text("Bullet list (- )")
                            .clicked()
                        {
                            action = Some(8);
                        }
                        if ui
                            .button("1.")
                            .on_hover_text("Ordered list (1. 2. 3.)")
                            .clicked()
                        {
                            action = Some(9);
                        }
                        if ui
                            .button("Task")
                            .on_hover_text("Task list (- [ ] )")
                            .clicked()
                        {
                            action = Some(10);
                        }
                        ui.separator();
                        if ui
                            .button("Link")
                            .on_hover_text("Link ([text](url))")
                            .clicked()
                        {
                            action = Some(11);
                        }
                        if ui
                            .button("Image")
                            .on_hover_text("Image (![alt](url))")
                            .clicked()
                        {
                            action = Some(12);
                        }
                        if ui
                            .button("Block")
                            .on_hover_text("Code block (```)")
                            .clicked()
                        {
                            action = Some(13);
                        }
                        if ui
                            .button("HR")
                            .on_hover_text("Horizontal rule (---)")
                            .clicked()
                        {
                            action = Some(14);
                        }
                        if ui.button("Table").on_hover_text("Insert table").clicked() {
                            action = Some(15);
                        }
                    });
                });
        });

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
