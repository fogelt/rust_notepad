use crate::app::NotepadApp;
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use std::sync::mpsc;

/// Build the prompt sent to Gemini: user instruction + current document.
fn build_prompt(instruction: &str, document: &str) -> String {
    format!(
        "You are helping edit a Markdown document in a notepad app.\n\
         Follow the user's instruction below. Return ONLY the resulting Markdown \
         (no code fences, no commentary) unless the user explicitly asks for an explanation.\n\n\
         --- USER INSTRUCTION ---\n\
         {instruction}\n\n\
         --- CURRENT DOCUMENT ---\n\
         {document}"
    )
}

/// Strip a wrapping ```markdown fence if the model added one anyway.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the first line (`markdown`, `md`).
        let mut lines = inner.lines();
        let first = lines.next().unwrap_or("");
        let first_trim = first.trim();
        if first_trim.eq_ignore_ascii_case("markdown") || first_trim.eq_ignore_ascii_case("md") {
            return lines.collect::<Vec<_>>().join("\n").trim().to_string();
        }
        return inner.trim().to_string();
    }
    trimmed.to_string()
}

fn gemini_generate(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.7, "maxOutputTokens": 2048 }
    });
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .query(&[("key", api_key)])
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("Gemini error {status}: {short}"));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Bad response from Gemini: {e}"))?;
    json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(strip_code_fence)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Gemini returned an empty response.".to_string())
}

fn insert_at_cursor(app: &mut NotepadApp, ctx: &egui::Context, snippet: &str) {
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

impl NotepadApp {
    fn poll_chat(&mut self) {
        if let Some(rx) = self.chat_rx.as_ref()
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(text) => {
                    self.chat_response = text;
                    self.chat_error = None;
                    self.status_message = String::from("Gemini suggestion ready.");
                }
                Err(err) => {
                    self.chat_error = Some(err);
                    self.status_message = String::from("Gemini request failed.");
                }
            }
            self.chat_rx = None;
            self.chat_busy = false;
        }
    }

    fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.is_empty() || self.chat_busy {
            return;
        }
        let api_key = match std::env::var("GEMINI_API_KEY") {
            Ok(k) if !k.trim().is_empty() => k,
            _ => {
                self.chat_error =
                    Some("GEMINI_API_KEY is not set. Add it to your .env file.".to_string());
                return;
            }
        };
        let model = match std::env::var("GEMINI_MODEL") {
            Ok(m) if !m.trim().is_empty() => m,
            _ => {
                self.chat_error =
                    Some("GEMINI_MODEL is not set. Add it to your .env file.".to_string());
                return;
            }
        };
        let prompt = build_prompt(&instruction, &self.content);
        let (tx, rx) = mpsc::channel();
        self.chat_rx = Some(rx);
        self.chat_busy = true;
        self.chat_error = None;
        self.chat_input.clear();
        self.status_message = String::from("Asking Gemini...");
        std::thread::spawn(move || {
            let result = gemini_generate(&prompt, &api_key, &model);
            let _ = tx.send(result);
        });
    }

    pub fn show_chat_window(&mut self, ctx: &egui::Context) {
        if !self.chat_open {
            return;
        }
        self.poll_chat();
        if self.chat_busy {
            ctx.request_repaint();
        }

        let mut open = self.chat_open;
        egui::Window::new("Gemini Assistant")
            .open(&mut open)
            .collapsible(false)
            .default_size([380.0, 440.0])
            .min_width(320.0)
            .show(ctx, |ui| {
                ui.label("Describe how to edit or write the Markdown:");
                let input = egui::TextEdit::multiline(&mut self.chat_input)
                    .hint_text("e.g. Turn these bullets into a table…")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .show(ui);
                // Ctrl+Enter while typing sends. Checked against the raw
                // input state (not `lost_focus`, which never fires here
                // because Enter keeps focus in a multiline field).
                if input.response.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)
                {
                    self.send_chat();
                }

                ui.horizontal(|ui| {
                    let send =
                        egui::Button::new(egui::RichText::new("✨ Send  (Ctrl+Enter)").strong())
                            .fill(egui::Color32::from_rgb(124, 92, 255));
                    ui.add_enabled(!self.chat_busy && !self.chat_input.trim().is_empty(), send)
                        .clicked()
                        .then(|| self.send_chat());
                    if self.chat_busy {
                        ui.spinner();
                        ui.label("Thinking…");
                    }
                });

                if let Some(err) = self.chat_error.clone() {
                    ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
                }

                ui.separator();
                ui.label(egui::RichText::new("Suggestion:").strong());
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if self.chat_response.is_empty() {
                            ui.weak("No suggestion yet.");
                        } else {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.chat_response)
                                    .desired_rows(8)
                                    .desired_width(f32::INFINITY)
                                    .code_editor(),
                            );
                        }
                    });

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.chat_response.is_empty(),
                            egui::Button::new("Insert at cursor"),
                        )
                        .clicked()
                    {
                        let snippet = self.chat_response.clone();
                        insert_at_cursor(self, ctx, &snippet);
                        self.status_message =
                            String::from("Unsaved changes (Save file to enable autosave).");
                    }
                    if ui
                        .add_enabled(
                            !self.chat_response.is_empty(),
                            egui::Button::new("Replace document"),
                        )
                        .clicked()
                    {
                        self.content = self.chat_response.clone();
                        self.status_message =
                            String::from("Unsaved changes (Save file to enable autosave).");
                    }
                    if ui.button("Clear").clicked() {
                        self.chat_response.clear();
                        self.chat_error = None;
                    }
                });
            });
        self.chat_open = open;
        if !open {
            // Dropped receiver (if any) just stops being polled; the
            // background thread's send will fail silently.
            self.chat_rx = None;
            self.chat_busy = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_instruction_and_document() {
        let p = build_prompt("Make it a table", "# Hi");
        assert!(p.contains("Make it a table"));
        assert!(p.contains("# Hi"));
        assert!(p.contains("Return ONLY"));
    }

    #[test]
    fn strips_markdown_fence_with_language_tag() {
        let out = strip_code_fence("```markdown\n# Hello\n```");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn strips_plain_fence_and_leaves_text_alone() {
        assert_eq!(strip_code_fence("```\nfoo\n```"), "foo");
        assert_eq!(strip_code_fence("# Hello"), "# Hello");
    }
}
