use crate::app::NotepadApp;
use crate::app::theme::{
    ACCENT, ACCENT_TEXT, ASSISTANT_DEFAULT_WIDTH, ASSISTANT_MIN_WIDTH, BG_ELEVATED, BORDER,
    MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH, panel_frame, panel_header,
};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui_phosphor::regular as icons;
use std::path::Path;
use std::sync::mpsc;

/// A small, curated set of models that work well for Markdown editing and
/// document dictation. Showing the whole live model catalog is overwhelming
/// and includes many unrelated models.
pub struct ModelChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
}

pub const MODELS: &[ModelChoice] = &[
    ModelChoice {
        id: "gemini-2.5-flash",
        label: "Gemini 2.5 Flash",
        note: "Recommended",
    },
    ModelChoice {
        id: "gemini-2.5-flash-lite",
        label: "Gemini 2.5 Flash Lite",
        note: "Fastest",
    },
    ModelChoice {
        id: "gemini-3.1-flash-lite",
        label: "Gemini 3.1 Flash Lite",
        note: "Higher free limit",
    },
];

pub fn default_model() -> &'static str {
    MODELS[0].id
}

pub fn model_label(id: &str) -> String {
    MODELS
        .iter()
        .find(|model| model.id == id)
        .map(|model| format!("{} · {}", model.label, model.note))
        .unwrap_or_else(|| id.to_string())
}

/// Gemini inline requests are capped around 20 MB; base64 adds ~33%, so keep
/// dropped files comfortably below that.
const MAX_AUDIO_BYTES: usize = 14 * 1024 * 1024;

fn audio_mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "wav" | "wave" => Some("audio/wav"),
        "mp3" => Some("audio/mp3"),
        "aiff" | "aif" => Some("audio/aiff"),
        "aac" => Some("audio/aac"),
        "ogg" | "oga" => Some("audio/ogg"),
        "flac" => Some("audio/flac"),
        "m4a" => Some("audio/mp4"),
        "webm" => Some("audio/webm"),
        _ => None,
    }
}

fn read_audio_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        std::fs::metadata(path).map_err(|err| format!("could not read the file: {err}"))?;
    if metadata.len() > MAX_AUDIO_BYTES as u64 {
        return Err(format!(
            "file is too large ({:.1} MB). Use an audio file under {} MB.",
            metadata.len() as f64 / 1_048_576.0,
            MAX_AUDIO_BYTES / 1_048_576
        ));
    }
    let bytes = std::fs::read(path).map_err(|err| format!("could not read the file: {err}"))?;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(format!(
            "file is too large. Use an audio file under {} MB.",
            MAX_AUDIO_BYTES / 1_048_576
        ));
    }
    Ok(bytes)
}

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
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.7, "maxOutputTokens": 2048 }
    });
    let json = post_generate(&body, api_key, model)?;
    let text =
        response_text(&json).ok_or_else(|| "Gemini returned an empty response.".to_string())?;
    Ok(strip_code_fence(&text))
}

/// Transcribe a WAV recording with the selected Gemini model. Gemini accepts
/// audio inline, so the existing assistant key/model can be reused.
fn gemini_transcribe(
    audio_bytes: &[u8],
    mime_type: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    use base64::Engine as _;
    let audio = base64::engine::general_purpose::STANDARD.encode(audio_bytes);
    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                {
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": audio
                    }
                },
                {
                    "text": "Transcribe the speech in this audio exactly as spoken. \
                             Return only the transcription text, with no commentary, labels, or quotation marks."
                }
            ]
        }],
        "generationConfig": { "temperature": 0.0, "maxOutputTokens": 1024 }
    });
    let json = post_generate(&body, api_key, model)?;
    response_text(&json)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Gemini returned an empty transcription.".to_string())
}

/// Join the text parts of the first candidate, skipping any "thought" parts.
fn response_text(json: &serde_json::Value) -> Option<String> {
    let parts = json["candidates"][0]["content"]["parts"].as_array()?;
    let text: String = parts
        .iter()
        .filter(|part| part.get("thought").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|part| part["text"].as_str())
        .collect();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn post_generate(
    body: &serde_json::Value,
    api_key: &str,
    model: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .query(&[("key", api_key)])
        .json(body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("Gemini error {status}: {short}"));
    }

    resp.json()
        .map_err(|e| format!("Bad response from Gemini: {e}"))
}

/// Replace the character range between `first` and `second` with dictated
/// `text`, adding spaces where it would otherwise run into neighbouring words.
/// Returns the new content and the cursor position (in chars) after the insert.
fn splice_dictation(content: &str, first: usize, second: usize, text: &str) -> (String, usize) {
    let (start_c, end_c) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let start_b = content
        .char_indices()
        .nth(start_c)
        .map(|(b, _)| b)
        .unwrap_or(content.len());
    let end_b = content
        .char_indices()
        .nth(end_c)
        .map(|(b, _)| b)
        .unwrap_or(content.len());

    let before = &content[..start_b];
    let after = &content[end_b..];
    let before_space = before
        .chars()
        .next_back()
        .is_some_and(|c| c.is_whitespace());
    let after_space = after.chars().next().is_some_and(|c| c.is_whitespace());

    let mut snippet = String::new();
    if !before.is_empty() && !before_space {
        snippet.push(' ');
    }
    snippet.push_str(text);
    if !after.is_empty() && !after_space {
        snippet.push(' ');
    }

    let cursor = before.chars().count() + snippet.chars().count();
    let mut updated = String::with_capacity(content.len() + snippet.len());
    updated.push_str(before);
    updated.push_str(&snippet);
    updated.push_str(after);
    (updated, cursor)
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

    /// Poll recording and transcription for document dictation. Called once
    /// per frame so it also runs while the Assistant panel is closed.
    pub fn poll_dictation(&mut self, ctx: &egui::Context) {
        self.poll_recording();
        self.poll_transcription(ctx);
        if self.recording.is_some() || self.transcribing {
            ctx.request_repaint();
        }
    }

    /// Poll the microphone thread; when the WAV is ready, transcribe it.
    fn poll_recording(&mut self) {
        let finished = self.recording.as_ref().and_then(|session| session.poll());
        let Some(result) = finished else {
            return;
        };
        self.recording = None;
        match result {
            Ok(wav) => self.start_transcription(wav, "audio/wav"),
            Err(err) => {
                self.status_message = format!("Dictation failed: {err}");
            }
        }
    }

    fn poll_transcription(&mut self, ctx: &egui::Context) {
        let received = match self.transcribe_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                Some(Err("Transcription worker stopped unexpectedly.".to_string()))
            }
            // Empty, or no request in flight.
            _ => None,
        };
        let Some(result) = received else {
            return;
        };
        self.transcribe_rx = None;
        self.transcribing = false;
        match result {
            Ok(text) => {
                let text = text.trim();
                if !text.is_empty() {
                    self.insert_dictation(ctx, text);
                    if self.file_path.is_none() {
                        self.status_message =
                            String::from("Dictation inserted — save the file to keep it.");
                    } else {
                        self.status_message = String::from("Dictation inserted.");
                    }
                } else {
                    self.status_message = String::from("No speech was detected.");
                }
            }
            Err(err) => {
                self.status_message = format!("Dictation failed: {err}");
            }
        }
    }

    /// Insert transcribed speech at the editor cursor, replacing any selection
    /// and keeping sensible spacing next to existing text.
    fn insert_dictation(&mut self, ctx: &egui::Context, text: &str) {
        let id = egui::Id::new("editor_text");
        let mut handled = false;

        if let Some(state) = TextEditState::load(ctx, id)
            && let Some(range) = state.cursor.char_range()
        {
            let (updated, new_c) = splice_dictation(
                &self.content,
                range.primary.index,
                range.secondary.index,
                text,
            );
            self.content = updated;
            if let Some(mut state) = TextEditState::load(ctx, id) {
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(new_c))));
                state.store(ctx, id);
            }
            handled = true;
        }

        if !handled {
            if !self.content.is_empty() && !self.content.ends_with(char::is_whitespace) {
                self.content.push(' ');
            }
            self.content.push_str(text);
        }
        ctx.memory_mut(|m| m.request_focus(id));
        self.note_content_changed();
    }

    fn start_transcription(&mut self, audio: Vec<u8>, mime: &'static str) {
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.status_message =
                String::from("Dictation needs a Gemini API key — add one in Assistant settings.");
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.status_message =
                String::from("Dictation needs a model — pick one in Assistant settings.");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.transcribe_rx = Some(rx);
        self.transcribing = true;
        self.status_message = String::from("Transcribing dictation with Gemini...");
        std::thread::spawn(move || {
            let _ = tx.send(gemini_transcribe(&audio, mime, &api_key, &model));
        });
    }

    /// Handle audio files dropped anywhere on the window: transcribe the first
    /// supported file and insert the result into the document.
    pub fn handle_dropped_audio(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| {
            i.raw.dropped_files.iter().find_map(|file| {
                let path = file.path.as_ref()?;
                audio_mime(path).map(|mime| (path.clone(), mime))
            })
        });
        let Some((path, mime)) = dropped else {
            return;
        };

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        if self.api_key().is_empty() {
            self.status_message = String::from(
                "Dropped audio needs a Gemini API key — add one in Assistant settings.",
            );
            return;
        }

        match read_audio_file(&path) {
            Ok(bytes) => {
                self.status_message = format!("Transcribing {name}…");
                self.start_transcription(bytes, mime);
            }
            Err(err) => {
                self.status_message = format!("Could not use {name}: {err}");
            }
        }
    }

    /// API key from the settings field, falling back to the environment.
    pub(crate) fn api_key(&self) -> String {
        if !self.chat_api_key.trim().is_empty() {
            return self.chat_api_key.trim().to_string();
        }
        crate::app::config::pick("", "GEMINI_API_KEY")
    }

    /// Send the instruction and the current document to Gemini.
    fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.is_empty() || self.chat_busy || self.transcribing || self.recording.is_some()
        {
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.chat_error = Some("No API key: add one in Assistant settings.".to_string());
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.chat_error =
                Some("No model selected. Pick one from the dropdown above.".to_string());
            return;
        }
        let prompt = build_prompt(&instruction, &self.content);
        let (tx, rx) = mpsc::channel();
        self.chat_rx = Some(rx);
        self.chat_busy = true;
        self.chat_error = None;
        self.chat_input.clear();
        self.status_message = String::from("Asking Gemini...");
        std::thread::spawn(move || {
            let _ = tx.send(gemini_generate(&prompt, &api_key, &model));
        });
    }

    /// API key and model live in a collapsible settings block so the
    /// instruction + suggestion stay front and center.
    fn show_assistant_settings(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.label(egui::RichText::new("API key").strong());
        ui.horizontal(|ui| {
            // Eye toggle lives inside the field, on the left; the widened
            // text margin keeps typed text clear.
            let mut text_margin = egui::Margin::symmetric(4.0, 2.0);
            text_margin.left = 30.0;
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.chat_api_key)
                    .password(!self.chat_show_key)
                    .hint_text("Paste your Gemini API key…")
                    .desired_width(f32::INFINITY)
                    .margin(text_margin),
            );
            let eye_rect = egui::Rect::from_min_size(
                field.rect.left_top() + egui::vec2(5.0, (field.rect.height() - 20.0) / 2.0),
                egui::vec2(20.0, 20.0),
            );
            let eye = if self.chat_show_key {
                icons::EYE
            } else {
                icons::EYE_SLASH
            };
            if ui
                .put(
                    eye_rect,
                    egui::Button::new(egui::RichText::new(eye).size(15.0)).frame(false),
                )
                .on_hover_text(if self.chat_show_key {
                    "Hide the key"
                } else {
                    "Show the key"
                })
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.chat_show_key = !self.chat_show_key;
            }
        });
        if !self.chat_api_key.trim().is_empty() && !self.chat_api_key.trim().starts_with("AIza") {
            ui.weak("Keys usually start with AIza — check for extra spaces.");
        }
        ui.horizontal(|ui| {
            if ui.button("Save key & model").clicked() {
                let cfg = crate::app::config::AssistantConfig {
                    api_key: self.chat_api_key.trim().to_string(),
                    model: self.chat_model.trim().to_string(),
                };
                match crate::app::config::save(&cfg) {
                    Ok(path) => {
                        self.chat_api_key = cfg.api_key;
                        self.chat_error = None;
                        self.status_message =
                            format!("Assistant settings saved to {}.", path.display());
                    }
                    Err(err) => self.chat_error = Some(err),
                }
            }
            ui.weak("Stored locally.");
        });
        ui.hyperlink_to(
            "Get a Gemini API key",
            "https://aistudio.google.com/api-keys",
        );

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Model").strong());
        let current = self.chat_model.clone();
        egui::ComboBox::from_id_source("gemini_model")
            .width(ui.available_width())
            .selected_text(model_label(&self.chat_model))
            .show_ui(ui, |ui| {
                for model in MODELS {
                    ui.selectable_value(
                        &mut self.chat_model,
                        model.id.to_string(),
                        format!("{} · {}", model.label, model.note),
                    );
                }
                if !MODELS.iter().any(|m| m.id == current.as_str()) && !current.is_empty() {
                    ui.separator();
                    ui.selectable_value(
                        &mut self.chat_model,
                        current.clone(),
                        format!("{current} (saved)"),
                    );
                }
            });
        ui.weak("Only models that work well for Markdown edits and dictation are listed.");

        ui.add_space(4.0);
    }

    pub fn show_chat_panel(&mut self, ctx: &egui::Context) {
        if !self.tabs.assistant {
            return;
        }

        self.poll_chat();
        if self.chat_busy {
            ctx.request_repaint();
        }

        let has_key = !self.api_key().is_empty();
        let settings_open = !has_key;
        let send_shortcut = ctx.format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Enter,
        ));

        egui::SidePanel::left("ai_chat_panel")
            .resizable(true)
            .default_width(ASSISTANT_DEFAULT_WIDTH)
            .min_width(ASSISTANT_MIN_WIDTH)
            // Cap the width so the panel can never be dragged wide enough
            // to hide the central editor entirely.
            .max_width((ctx.available_rect().width() - MIN_EDITOR_WIDTH).max(MIN_SIDE_PANEL_WIDTH))
            .frame(panel_frame(ctx))
            .show(ctx, |ui| {
                if panel_header(ui, "Assistant", "Close").clicked() {
                    self.tabs.assistant = false;
                }
                ui.separator();
                // Roomier spacing, and everything below scrolls so short
                // windows clip nothing instead of hiding content.
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                let total_height = ui.available_height();
                let top_max = (total_height - 190.0).max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(top_max)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(2.0);

                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!("{} Settings", icons::GEAR_SIX)).strong(),
                        )
                        .default_open(settings_open)
                        .show(ui, |ui| self.show_assistant_settings(ui));

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Instruction").strong());
                            if !self.chat_input.is_empty() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Clear").clicked() {
                                            self.chat_input.clear();
                                        }
                                    },
                                );
                            }
                        });

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

                        let can_send =
                            !self.chat_busy && !self.chat_input.trim().is_empty();

                        ui.horizontal_wrapped(|ui| {
                            let send = egui::Button::new(
                                egui::RichText::new(format!(
                                    "{} Send  ({send_shortcut})",
                                    icons::PAPER_PLANE_TILT
                                ))
                                .strong()
                                .color(ACCENT_TEXT),
                            )
                            .fill(ACCENT);
                            if ui
                                .add_enabled(can_send, send)
                                .on_hover_text("Send the instruction to Gemini")
                                .clicked()
                            {
                                self.send_chat();
                            }

                            if self.chat_busy {
                                ui.spinner();
                                ui.label("Thinking…");
                            }
                        });

                        if !has_key {
                            ui.weak("Add a Gemini API key in Settings to use the assistant.");
                        } else {
                            ui.weak(
                                "Use the microphone in the editor toolbar to dictate into the document.",
                            );
                        }

                        if let Some(err) = self.chat_error.clone() {
                            let err_color = ui.visuals().error_fg_color;
                            ui.colored_label(err_color, err);
                        }

                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Suggestion").strong());
                    if !self.chat_response.is_empty() {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.weak(
                                    egui::RichText::new(model_label(&self.chat_model)).small(),
                                );
                            },
                        );
                    }
                });
                ui.add_space(4.0);

                let card_height = ui.available_height().max(140.0);
                egui::Frame::none()
                    .fill(BG_ELEVATED)
                    .rounding(8.0)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_min_height((card_height - 20.0).max(100.0));
                        let actions_height = if self.chat_response.is_empty() { 0.0 } else { 36.0 };
                        let body_height = (ui.available_height() - actions_height).max(60.0);
                        if self.chat_response.is_empty() {
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), body_height),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.add_space(((body_height - 46.0) * 0.5).max(8.0));
                                    ui.label(
                                        egui::RichText::new(icons::SPARKLE)
                                            .size(22.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Gemini's suggestion will appear here.",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                },
                            );
                        } else {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.add_sized(
                                        egui::vec2(ui.available_width(), body_height),
                                        egui::TextEdit::multiline(&mut self.chat_response)
                                            .desired_width(f32::INFINITY)
                                            .frame(false)
                                            .code_editor(),
                                    );
                                });
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Insert at cursor").clicked() {
                                    let snippet = self.chat_response.clone();
                                    insert_at_cursor(self, ctx, &snippet);
                                    self.note_content_changed();
                                }
                                if ui.button("Replace document").clicked() {
                                    self.content = self.chat_response.clone();
                                    self.note_content_changed();
                                }
                                if ui.button("Clear").clicked() {
                                    self.chat_response.clear();
                                    self.chat_error = None;
                                }
                            });
                        }
                    });

            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictation_adds_spacing_between_words() {
        let (text, cursor) = splice_dictation("hello world", 5, 5, "there");
        assert_eq!(text, "hello there world");
        assert_eq!(cursor, "hello there".chars().count());
    }

    #[test]
    fn dictation_replaces_the_selection() {
        let (text, cursor) = splice_dictation("keep this and that", 5, 9, "only");
        assert_eq!(text, "keep only and that");
        assert_eq!(cursor, "keep only".chars().count());
    }

    #[test]
    fn dictation_handles_reversed_and_empty_selection() {
        let (text, _) = splice_dictation("ab", 2, 0, "X");
        assert_eq!(text, "X");
        let (text, cursor) = splice_dictation("", 0, 0, "first");
        assert_eq!(text, "first");
        assert_eq!(cursor, 5);
    }

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

    #[test]
    fn curated_models_are_unique_and_usable() {
        assert!(!MODELS.is_empty());
        assert!(MODELS.iter().any(|m| m.id == default_model()));
        let mut ids: Vec<_> = MODELS.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MODELS.len());
        for model in MODELS {
            assert!(!model.label.is_empty());
            assert!(!model.note.is_empty());
        }
    }

    #[test]
    fn model_labels_are_friendly() {
        assert!(model_label("gemini-2.5-flash").contains("Recommended"));
        assert_eq!(model_label("custom-model"), "custom-model");
    }

    #[test]
    fn detects_supported_audio_files() {
        assert_eq!(audio_mime(Path::new("memo.WAV")), Some("audio/wav"));
        assert_eq!(audio_mime(Path::new("memo.mp3")), Some("audio/mp3"));
        assert_eq!(audio_mime(Path::new("memo.m4a")), Some("audio/mp4"));
        assert_eq!(audio_mime(Path::new("memo.flac")), Some("audio/flac"));
    }

    #[test]
    fn ignores_non_audio_files() {
        assert_eq!(audio_mime(Path::new("notes.md")), None);
        assert_eq!(audio_mime(Path::new("picture.png")), None);
        assert_eq!(audio_mime(Path::new("no-extension")), None);
    }

    #[test]
    fn joins_text_parts_and_skips_thoughts() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "thinking..."},
                        {"text": "make it "},
                        {"text": "a table"}
                    ]
                }
            }]
        });
        assert_eq!(response_text(&json).as_deref(), Some("make it a table"));
    }

    #[test]
    fn reads_audio_files_and_rejects_large_ones() {
        let dir = std::env::temp_dir().join(format!("rust_notepad_audio_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let small = dir.join("memo.wav");
        std::fs::write(&small, b"RIFF....").unwrap();
        assert_eq!(read_audio_file(&small).unwrap(), b"RIFF....");

        let huge = dir.join("huge.wav");
        let file = std::fs::File::create(&huge).unwrap();
        file.set_len(MAX_AUDIO_BYTES as u64 + 1).unwrap();
        let err = read_audio_file(&huge).unwrap_err();
        assert!(err.contains("too large"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
