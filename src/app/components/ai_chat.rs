use crate::app::NotepadApp;
use crate::app::live::LiveEvent;
use crate::app::models::{DictationEvent, DictationStream};
use crate::app::theme::{
    ACCENT, ACCENT_TEXT, ASSISTANT_DEFAULT_WIDTH, ASSISTANT_MIN_WIDTH, BG_ELEVATED, BORDER,
    MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH, panel_frame, panel_header,
};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui_phosphor::regular as icons;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Draw a dashed straight line from `a` to `b`.
fn dashed_line(painter: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: egui::Stroke) {
    const DASH: f32 = 10.0;
    const GAP: f32 = 6.0;
    let delta = b - a;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = delta / length;
    let mut offset = 0.0;
    while offset < length {
        let end = (offset + DASH).min(length);
        painter.line_segment([a + direction * offset, a + direction * end], stroke);
        offset = end + GAP;
    }
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

/// Stream a transcription with the selected Gemini model, forwarding text
/// deltas to the UI as they arrive. Gemini accepts audio inline, so the
/// existing assistant key/model can be reused.
fn stream_transcription(
    audio_bytes: &[u8],
    mime_type: &str,
    api_key: &str,
    model: &str,
    tx: &mpsc::Sender<DictationEvent>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    use base64::Engine as _;
    use std::io::BufRead as _;

    let audio = base64::engine::general_purpose::STANDARD.encode(audio_bytes);
    let mut generation = serde_json::json!({
        "temperature": 0.0,
        "maxOutputTokens": 1024
    });
    // Transcription needs no reasoning. Turning thinking off makes Gemini
    // start emitting the transcript immediately (and stream it) instead of
    // buffering thought tokens first.
    if model.contains("2.5-flash") {
        generation["thinkingConfig"] = serde_json::json!({ "thinkingBudget": 0 });
    }
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
        "generationConfig": generation
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent",
        model.trim()
    );
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .query(&[("key", api_key), ("alt", "sse")])
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("Gemini error {status}: {short}"));
    }

    let reader = std::io::BufReader::new(resp);
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let line = line.map_err(|e| format!("Could not read the Gemini stream: {e}"))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let json: serde_json::Value = serde_json::from_str(data)
            .map_err(|e| format!("Bad streaming response from Gemini: {e}"))?;
        if let Some(message) = json
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
        {
            return Err(message.to_string());
        }
        if let Some(text) = response_text(&json) {
            let _ = tx.send(DictationEvent::Delta(text));
        }
    }
    Ok(())
}

/// Turn raw Gemini/network errors into something a person can act on.
fn friendly_gemini_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("api key")
        || lower.contains("api_key")
        || lower.contains("unauthenticated")
        || lower.contains("permission")
        || lower.contains("401")
        || lower.contains("403")
    {
        return String::from(
            "Gemini rejected the API key. Check it in Assistant settings and try again.",
        );
    }
    if lower.contains("429")
        || lower.contains("quota")
        || lower.contains("rate limit")
        || lower.contains("resource_exhausted")
    {
        return String::from(
            "Gemini is rate limiting requests right now. Wait a moment and try again.",
        );
    }
    if lower.contains("404") || lower.contains("not found") || lower.contains("not supported") {
        return String::from(
            "That Gemini model isn't available for dictation. Pick another model in Assistant settings.",
        );
    }
    if lower.contains("live") && (lower.contains("closed") || lower.contains("acknowledge")) {
        return String::from(
            "Gemini Live couldn't start. Check your API key in Assistant settings and try again.",
        );
    }
    if lower.contains("request failed")
        || lower.contains("sending request")
        || lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("could not connect")
    {
        return String::from(
            "Could not reach Gemini. Check your internet connection and try again.",
        );
    }
    let short = raw.chars().take(220).collect::<String>();
    format!("Dictation failed: {short}")
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

/// Prepare an edit for streamed dictation: replace the selected character
/// range, add a leading space if it would run into the previous word, and
/// report where deltas should be inserted. The returned `bool` says whether a
/// trailing space is needed once the stream finishes.
fn prepare_dictation(content: &str, first: usize, second: usize) -> (String, usize, bool) {
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

    let mut updated = String::with_capacity(content.len() + 1);
    updated.push_str(before);
    if !before.is_empty() && !before_space {
        updated.push(' ');
    }
    let insert_at = updated.len();
    updated.push_str(after);
    let needs_trailing = !after.is_empty() && !after_space;
    (updated, insert_at, needs_trailing)
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
        self.poll_live(ctx);
        self.poll_transcription(ctx);
        self.reveal_dictation(ctx);
        if self.is_dictating() {
            ctx.request_repaint();
        }
    }

    /// Poll the Live API session for interim and final transcripts.
    fn poll_live(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(session) = self.live.as_ref() {
            while let Some(event) = session.poll() {
                let terminal = matches!(event, LiveEvent::Error(_) | LiveEvent::Closed);
                events.push(event);
                if terminal {
                    break;
                }
            }
        }
        for event in events {
            match event {
                LiveEvent::Interim(text) => self.apply_live_interim(ctx, &text),
                LiveEvent::Final(text) => self.apply_live_final(ctx, &text),
                LiveEvent::Error(err) => {
                    self.fail_live(ctx, &err);
                    return;
                }
                LiveEvent::Closed => {
                    self.finish_live(ctx);
                    return;
                }
            }
        }
    }

    /// Show an updated partial transcript, replacing the previous hypothesis.
    fn apply_live_interim(&mut self, ctx: &egui::Context, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }

        let mut cursor = 0;
        let mut changed = false;
        if let Some(stream) = self.dictation_stream.as_mut() {
            let separator = if !stream.inserted.is_empty()
                && !stream
                    .inserted
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_whitespace())
                && !text.chars().next().is_some_and(|c| c.is_whitespace())
            {
                " "
            } else {
                ""
            };
            let next = format!("{separator}{text}");
            if next == stream.interim {
                return;
            }

            let start = stream.start_b + stream.inserted.len();
            let end = start + stream.interim.len();
            if end > self.content.len() {
                return;
            }
            self.content.replace_range(start..end, &next);
            stream.interim = next;
            let end = start + stream.interim.len();
            cursor = self.content[..end].chars().count();
            changed = true;
        }

        if changed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
    }

    /// Commit a finalized segment, replacing the last interim hypothesis.
    fn apply_live_final(&mut self, ctx: &egui::Context, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }

        let mut cursor = 0;
        let mut changed = false;
        if let Some(stream) = self.dictation_stream.as_mut() {
            let start = stream.start_b + stream.inserted.len();
            let end = start + stream.interim.len();
            if end > self.content.len() {
                return;
            }
            self.content.replace_range(start..end, "");
            stream.interim.clear();

            let separator = if !stream.inserted.is_empty()
                && !stream
                    .inserted
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_whitespace())
                && !text.chars().next().is_some_and(|c| c.is_whitespace())
            {
                " "
            } else {
                ""
            };
            let commit = format!("{separator}{text}");
            let at = stream.start_b + stream.inserted.len();
            self.content.insert_str(at, &commit);
            stream.inserted.push_str(&commit);
            let end = stream.start_b + stream.inserted.len();
            cursor = self.content[..end].chars().count();
            changed = true;
        }

        if changed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
    }

    /// Finish a live session, keeping any last interim text if no final arrived.
    fn finish_live(&mut self, ctx: &egui::Context) {
        if self.live.is_none() && self.dictation_stream.is_none() {
            return;
        }
        self.live = None;
        self.live_finishing = false;
        if let Some(stream) = self.dictation_stream.as_mut()
            && !stream.interim.is_empty()
        {
            stream.inserted.push_str(&stream.interim);
            stream.interim.clear();
        }
        self.finish_transcription(ctx);
    }

    /// Abort a live session, roll back partial text, and toast the error.
    fn fail_live(&mut self, ctx: &egui::Context, raw: &str) {
        if let Some(session) = self.live.take() {
            session.request_stop();
        }
        self.live_finishing = false;
        self.fail_transcription(ctx, raw);
    }

    /// Drain streamed dictation events. Several deltas can arrive per frame.
    fn poll_transcription(&mut self, ctx: &egui::Context) {
        while let Some(event) = self.next_dictation_event() {
            match event {
                DictationEvent::Delta(text) => self.apply_dictation_delta(ctx, &text),
                DictationEvent::Finished => {
                    self.transcribe_rx = None;
                    self.transcribe_cancel = None;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        // Keep revealing the buffered text, then finish.
                        stream.finished = true;
                    } else {
                        self.finish_transcription(ctx);
                    }
                    break;
                }
                DictationEvent::Failed(err) => {
                    self.fail_transcription(ctx, &err);
                    break;
                }
            }
        }
    }

    fn next_dictation_event(&mut self) -> Option<DictationEvent> {
        let rx = self.transcribe_rx.as_ref()?;
        match rx.try_recv() {
            Ok(event) => Some(event),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(DictationEvent::Failed(
                "The transcription worker stopped unexpectedly.".to_string(),
            )),
        }
    }

    /// Buffer a streamed chunk. [`Self::reveal_dictation`] feeds it into the
    /// document a few characters at a time.
    fn apply_dictation_delta(&mut self, ctx: &egui::Context, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }
        if let Some(stream) = self.dictation_stream.as_mut() {
            stream.pending.push_str(delta);
        }
    }

    /// Move pending characters into the document each frame. Small deltas are
    /// shown immediately; large bursts are revealed progressively so text
    /// never pops in all at once.
    fn reveal_dictation(&mut self, ctx: &egui::Context) {
        let mut revealed = false;
        let mut cursor = 0;
        let mut done = false;

        if let Some(stream) = self.dictation_stream.as_mut() {
            let pending = stream.pending.chars().count();
            if pending > 0 {
                let take = if pending <= 32 {
                    pending
                } else {
                    (pending / 10).clamp(4, 16)
                };
                let split = stream
                    .pending
                    .char_indices()
                    .nth(take)
                    .map(|(index, _)| index)
                    .unwrap_or(stream.pending.len());
                let piece = stream.pending[..split].to_string();
                stream.pending.drain(..split);

                let at = stream.start_b + stream.inserted.len();
                self.content.insert_str(at, &piece);
                stream.inserted.push_str(&piece);
                let end = stream.start_b + stream.inserted.len();
                cursor = self.content[..end].chars().count();
                revealed = true;
            }
            done = stream.finished && stream.pending.is_empty();
        }

        if revealed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
        if done {
            self.finish_transcription(ctx);
        }
    }

    /// Snapshot the document and remove the selection before the first delta.
    fn begin_dictation_stream(&mut self, ctx: &egui::Context) {
        let range = TextEditState::load(ctx, egui::Id::new("editor_text"))
            .and_then(|state| state.cursor.char_range());
        let original_content = self.content.clone();
        let (start_c, end_c) = match range {
            Some(range) => {
                let (a, b) = (range.primary.index, range.secondary.index);
                if a <= b { (a, b) } else { (b, a) }
            }
            None => {
                let end = self.content.chars().count();
                (end, end)
            }
        };
        let (prepared, start_b, after_needs_space) =
            prepare_dictation(&self.content, start_c, end_c);
        self.content = prepared;
        self.refresh_stats();
        self.dictation_stream = Some(DictationStream {
            original_content,
            original_range: range.map(|range| (range.primary.index, range.secondary.index)),
            start_b,
            inserted: String::new(),
            pending: String::new(),
            interim: String::new(),
            after_needs_space,
            finished: false,
        });
    }

    fn finish_transcription(&mut self, ctx: &egui::Context) {
        self.transcribe_rx = None;
        self.transcribe_cancel = None;
        self.transcribing = false;

        let Some(stream) = self.dictation_stream.take() else {
            self.status_message = String::from("No speech was detected.");
            self.focus_editor(ctx);
            return;
        };
        if stream.inserted.trim().is_empty() {
            self.content = stream.original_content;
            self.restore_editor_range(ctx, stream.original_range);
            self.refresh_stats();
            self.status_message = String::from("No speech was detected.");
            self.focus_editor(ctx);
            return;
        }

        if stream.after_needs_space {
            let at = stream.start_b + stream.inserted.len();
            self.content.insert(at, ' ');
        }
        let end = stream.start_b + stream.inserted.len();
        let cursor = self.content[..end].chars().count();
        self.store_editor_cursor(ctx, cursor);
        self.note_content_changed();
        self.status_message = if self.file_path.is_none() {
            String::from("Dictation inserted — save the file to keep it.")
        } else {
            String::from("Dictation inserted.")
        };
        self.focus_editor(ctx);
    }

    /// Abort the stream, roll back any partial text, and toast the error.
    fn fail_transcription(&mut self, ctx: &egui::Context, raw: &str) {
        self.transcribe_rx = None;
        self.transcribe_cancel = None;
        self.transcribing = false;

        if let Some(stream) = self.dictation_stream.take() {
            self.content = stream.original_content;
            self.restore_editor_range(ctx, stream.original_range);
            self.refresh_stats();
        }

        let message = friendly_gemini_error(raw);
        self.status_message = message.clone();
        self.push_toast(message, true);
        self.focus_editor(ctx);
    }

    /// Stop the microphone and cancel any stream in flight, rolling back the
    /// partially streamed text.
    pub fn cancel_dictation(&mut self, ctx: &egui::Context) {
        if let Some(session) = self.live.take() {
            session.request_stop();
        }
        self.live_finishing = false;
        if let Some(cancel) = self.transcribe_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.transcribe_rx = None;
        self.transcribing = false;
        if let Some(stream) = self.dictation_stream.take() {
            self.content = stream.original_content;
            self.restore_editor_range(ctx, stream.original_range);
            self.refresh_stats();
        }
    }

    fn store_editor_cursor(&self, ctx: &egui::Context, char_index: usize) {
        let id = egui::Id::new("editor_text");
        if let Some(mut state) = TextEditState::load(ctx, id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(
                    char_index,
                ))));
            state.store(ctx, id);
        }
    }

    fn focus_editor(&self, ctx: &egui::Context) {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("editor_text")));
    }

    fn restore_editor_range(&self, ctx: &egui::Context, range: Option<(usize, usize)>) {
        let id = egui::Id::new("editor_text");
        if let Some(mut state) = TextEditState::load(ctx, id) {
            let range = range.map(|(a, b)| {
                CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))
            });
            state.cursor.set_char_range(range);
            state.store(ctx, id);
        }
    }

    /// Start real-time dictation over the Gemini Live API.
    pub fn start_live_dictation(&mut self, ctx: &egui::Context) {
        let api_key = self.api_key();
        if api_key.is_empty() {
            let message = String::from("Add a Gemini API key in Assistant settings to dictate.");
            self.status_message = message.clone();
            self.push_toast(message, true);
            return;
        }

        self.cancel_dictation(ctx);
        self.live = Some(crate::app::live::start(api_key));
        self.live_finishing = false;
        self.status_message = String::from("Listening… speak, then click to stop.");
    }

    /// Stop the live microphone and wait for the final transcript.
    pub fn stop_live_dictation(&mut self) {
        if let Some(session) = self.live.as_ref() {
            session.request_stop();
            self.live_finishing = true;
            self.status_message = String::from("Finishing dictation…");
        }
    }

    fn start_transcription(&mut self, ctx: &egui::Context, audio: Vec<u8>, mime: &'static str) {
        self.cancel_dictation(ctx);

        let api_key = self.api_key();
        if api_key.is_empty() {
            let message = String::from("Add a Gemini API key in Assistant settings to dictate.");
            self.status_message = message.clone();
            self.push_toast(message, true);
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            let message = String::from("Pick a Gemini model in Assistant settings to dictate.");
            self.status_message = message.clone();
            self.push_toast(message, true);
            return;
        }

        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.transcribe_rx = Some(rx);
        self.transcribe_cancel = Some(Arc::clone(&cancel));
        self.transcribing = true;
        self.dictation_stream = None;
        self.status_message = String::from("Transcribing dictation with Gemini...");
        std::thread::spawn(move || {
            let result = stream_transcription(&audio, mime, &api_key, &model, &tx, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let event = match result {
                Ok(()) => DictationEvent::Finished,
                Err(err) => DictationEvent::Failed(err),
            };
            let _ = tx.send(event);
        });
    }

    /// Draw a dashed accent drop target while an audio file is dragged over
    /// the window.
    pub fn show_audio_drop_overlay(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| {
            i.raw.hovered_files.iter().any(|file| {
                file.path
                    .as_ref()
                    .is_some_and(|path| audio_mime(path).is_some())
            })
        });
        if !hovering {
            return;
        }

        let rect = ctx.screen_rect().shrink(16.0);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("audio_drop_overlay"),
        ));
        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 20),
        );

        let border = rect.shrink(1.0);
        let stroke = egui::Stroke::new(2.0_f32, ACCENT);
        let corners = [
            border.left_top(),
            border.right_top(),
            border.right_bottom(),
            border.left_bottom(),
            border.left_top(),
        ];
        for pair in corners.windows(2) {
            dashed_line(&painter, pair[0], pair[1], stroke);
        }

        let galley = painter.layout_no_wrap(
            String::from("Import audio file"),
            egui::FontId::proportional(19.0),
            ACCENT_TEXT,
        );
        painter.galley(rect.center() - galley.size() * 0.5, galley, ACCENT_TEXT);
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
            let message =
                String::from("Add a Gemini API key in Assistant settings to transcribe audio.");
            self.status_message = message.clone();
            self.push_toast(message, true);
            return;
        }

        match read_audio_file(&path) {
            Ok(bytes) => {
                self.status_message = format!("Transcribing {name}…");
                self.start_transcription(ctx, bytes, mime);
            }
            Err(err) => {
                let message = format!("Could not use {name}: {err}");
                self.status_message = message.clone();
                self.push_toast(message, true);
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
        if instruction.is_empty() || self.chat_busy || self.transcribing || self.live.is_some() {
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
                            let dictating = self.is_dictating();
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Insert at cursor"))
                                    .clicked()
                                {
                                    let snippet = self.chat_response.clone();
                                    insert_at_cursor(self, ctx, &snippet);
                                    self.note_content_changed();
                                }
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Replace document"))
                                    .clicked()
                                {
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
    fn dictation_inserts_with_spacing_between_words() {
        let (text, at, trailing) = prepare_dictation("hello world", 5, 5);
        assert_eq!(text, "hello  world");
        assert_eq!(at, 6);
        assert!(!trailing);
    }

    #[test]
    fn dictation_replaces_the_selection() {
        let (text, at, trailing) = prepare_dictation("keep this and that", 5, 9);
        assert_eq!(text, "keep  and that");
        assert_eq!(at, 5);
        assert!(!trailing);
        let mut streamed = text;
        streamed.insert_str(at, "only");
        assert_eq!(streamed, "keep only and that");
    }

    #[test]
    fn dictation_handles_reversed_and_empty_selection() {
        let (text, at, trailing) = prepare_dictation("ab", 2, 0);
        assert_eq!(text, "");
        assert_eq!(at, 0);
        assert!(!trailing);
        let (text, at, trailing) = prepare_dictation("", 0, 0);
        assert_eq!(text, "");
        assert_eq!(at, 0);
        assert!(!trailing);
    }

    #[test]
    fn dictation_reserves_a_trailing_space() {
        let (_, at, trailing) = prepare_dictation("ab", 1, 1);
        assert_eq!(at, 2);
        assert!(trailing);
    }

    #[test]
    fn friendly_errors_point_at_the_fix() {
        assert!(friendly_gemini_error("403 API key not valid").contains("API key"));
        assert!(friendly_gemini_error("429 RESOURCE_EXHAUSTED").contains("rate limiting"));
        assert!(friendly_gemini_error("Request failed: dns error").contains("internet"));
        assert!(friendly_gemini_error("weird thing").contains("weird thing"));
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
