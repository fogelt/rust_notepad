//! Microphone dictation and transcription state.
//!
//! Local Whisper sessions and provider streaming sessions both feed the same
//! streamed-edit logic here, keeping the editor buffer in sync. The assistant
//! UI and report generation live in `components::ai_chat`.

use crate::app::NotepadApp;
use crate::app::ai::provider::Provider;
use crate::app::ai::text;
use crate::app::local_stt::{self, LocalSession};
use crate::app::models::{DictationEvent, DictationSource, DictationStream};
use crate::app::provider_stt::{self, ProviderSession};
use crate::app::theme::{ACCENT, ACCENT_TEXT};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
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

/// Message shown when the active provider has no transcription API.
fn unsupported_transcription(provider: Provider) -> String {
    format!(
        "{} doesn't offer transcription. Choose Local in Settings > Dictation or \
         switch to Gemini or OpenAI in Settings > Provider.",
        provider.label()
    )
}

/// Message shown when the provider needs an API key for dictation.
fn missing_transcription_key(provider: Provider, action: &str) -> String {
    format!(
        "Add a {} API key in Settings > Provider to {action}.",
        provider.label()
    )
}
impl NotepadApp {
    /// Drain offline dictation deltas. Local Whisper commits each chunk as it
    /// finishes, so every delta is final text.
    pub(crate) fn poll_local_dictation(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(session) = self.local_dictation.as_ref() {
            while let Some(event) = session.poll() {
                let terminal =
                    matches!(event, DictationEvent::Finished | DictationEvent::Failed(_));
                events.push(event);
                if terminal {
                    break;
                }
            }
        }
        for event in events {
            match event {
                DictationEvent::Delta(text) => self.apply_dictation_delta(ctx, &text),
                DictationEvent::Interim(text) => self.apply_dictation_interim(ctx, &text),
                DictationEvent::Final(text) => self.apply_dictation_final(ctx, &text),
                DictationEvent::Finished => {
                    self.local_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    } else {
                        self.finish_transcription(ctx);
                    }
                    self.status_message = String::from("Dictation inserted.");
                }
                DictationEvent::Failed(err) => {
                    self.local_dictation = None;
                    self.dictation_stopping = false;
                    // Keep whatever was already transcribed; only report the
                    // failure instead of rolling the text back.
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    }
                    self.status_message = String::from("Local dictation stopped.");
                    self.push_toast(text::friendly_local_error(&err), true);
                    break;
                }
            }
        }
        if self.local_dictation.is_some() {
            ctx.request_repaint();
        }
    }

    /// Drain provider dictation deltas and finish the edit when it stops.
    pub(crate) fn poll_provider_dictation(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(session) = self.provider_dictation.as_ref() {
            while let Some(event) = session.poll() {
                let terminal =
                    matches!(event, DictationEvent::Finished | DictationEvent::Failed(_));
                events.push(event);
                if terminal {
                    break;
                }
            }
        }
        for event in events {
            match event {
                DictationEvent::Delta(text) => self.apply_dictation_delta(ctx, &text),
                DictationEvent::Interim(text) => self.apply_dictation_interim(ctx, &text),
                DictationEvent::Final(text) => self.apply_dictation_final(ctx, &text),
                DictationEvent::Finished => {
                    self.provider_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    } else {
                        self.finish_transcription(ctx);
                    }
                    self.status_message = String::from("Dictation inserted.");
                }
                DictationEvent::Failed(err) => {
                    self.provider_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    }
                    self.status_message = String::from("Provider dictation stopped.");
                    self.push_toast(
                        text::friendly_generation_error(&err, self.chat_provider),
                        true,
                    );
                    break;
                }
            }
        }
        if self.provider_dictation.is_some() {
            ctx.request_repaint();
        }
    }

    /// Poll the local model download and report when it finishes.
    pub(crate) fn poll_whisper_download(&mut self, ctx: &egui::Context) {
        let mut outcome = None;
        let mut finished_model = None;
        if let Some(download) = self.whisper_download.as_mut() {
            while let Some(event) = download.poll() {
                match event {
                    local_stt::DownloadEvent::Progress { .. } => {}
                    local_stt::DownloadEvent::Ready => {
                        finished_model = Some(download.model);
                        outcome = Some(Ok(()));
                        break;
                    }
                    local_stt::DownloadEvent::Failed(err) => {
                        outcome = Some(Err(err));
                        break;
                    }
                }
            }
        }
        if let Some(result) = outcome {
            self.whisper_download = None;
            match result {
                Ok(()) => {
                    if let Some(model) = finished_model {
                        self.dictation_model = model.id.to_string();
                        self.save_assistant_settings();
                    }
                    self.whisper_error = None;
                    self.status_message = String::from("Local dictation model ready.");
                    self.push_toast("Local dictation model ready.", false);
                }
                Err(err) => {
                    self.whisper_error = Some(err.clone());
                    self.push_toast(err, true);
                }
            }
        }
        if self.whisper_download.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
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

    /// Show an updated speculative transcript, replacing the previous interim.
    fn apply_dictation_interim(&mut self, ctx: &egui::Context, text: &str) {
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

    /// Commit a finalized segment, replacing the current interim text.
    fn apply_dictation_final(&mut self, ctx: &egui::Context, text: &str) {
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

    /// Move pending characters into the document each frame. Small deltas are
    /// shown immediately; large bursts are revealed progressively so text
    /// never pops in all at once.
    pub(crate) fn reveal_dictation(&mut self, ctx: &egui::Context) {
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
        let Some(mut stream) = self.dictation_stream.take() else {
            self.status_message = String::from("No speech was detected.");
            self.focus_editor(ctx);
            return;
        };
        if !stream.interim.is_empty() {
            stream.inserted.push_str(&stream.interim);
            stream.interim.clear();
        }
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

    /// Stop the worker and roll back the partially streamed text.
    pub fn cancel_dictation(&mut self, ctx: &egui::Context) {
        if let Some(session) = self.local_dictation.take() {
            session.request_stop();
        }
        if let Some(session) = self.provider_dictation.take() {
            session.request_stop();
        }
        self.dictation_stopping = false;
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

    /// Start microphone dictation with the selected source.
    pub fn start_dictation(&mut self, ctx: &egui::Context) {
        match self.dictation_source {
            DictationSource::Provider => self.start_provider_dictation(ctx),
            DictationSource::Local => self.start_local_dictation(ctx),
        }
    }

    fn start_provider_dictation(&mut self, ctx: &egui::Context) {
        if !provider_stt::supports(self.chat_provider) {
            self.report_error(unsupported_transcription(self.chat_provider));
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error(missing_transcription_key(self.chat_provider, "dictate"));
            return;
        }
        let model = provider_stt::transcription_model(self.chat_provider).to_string();
        if model.is_empty() {
            self.report_error("This provider has no transcription model.");
            return;
        }
        let provider = self.chat_provider;
        self.cancel_dictation(ctx);
        self.provider_dictation = Some(ProviderSession::start_microphone(provider, api_key, model));
        self.dictation_stopping = false;
        self.status_message = format!("Listening with {}…", provider.label());
    }

    fn start_local_dictation(&mut self, ctx: &egui::Context) {
        let Some(paths) = self.dictation_model_paths() else {
            self.report_error(
                "Download a dictation model in Settings > Dictation to use the microphone.",
            );
            return;
        };
        self.cancel_dictation(ctx);
        self.local_dictation = Some(LocalSession::start_microphone(paths));
        self.dictation_stopping = false;
        self.status_message = String::from("Listening… speak, then click to stop.");
    }

    /// Stop the microphone and let the worker transcribe its last audio.
    pub fn stop_dictation(&mut self) {
        if let Some(session) = self.local_dictation.as_ref() {
            session.request_stop();
            self.dictation_stopping = true;
            self.status_message = String::from("Finishing dictation…");
            return;
        }
        if let Some(session) = self.provider_dictation.as_ref() {
            session.request_stop();
            self.dictation_stopping = true;
            self.status_message = String::from("Finishing dictation…");
        }
    }

    /// Highlight the window while an audio file is dragged over it.
    pub fn show_audio_drop_overlay(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| {
            i.raw.hovered_files.iter().any(|file| {
                file.path
                    .as_ref()
                    .is_some_and(|path| local_stt::is_audio_file(path))
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
            10.0,
            egui::Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 24),
        );
        painter.rect_stroke(rect.shrink(1.0), 10.0, egui::Stroke::new(2.0_f32, ACCENT));

        let galley = painter.layout_no_wrap(
            String::from("Transcribe audio file"),
            egui::FontId::proportional(19.0),
            ACCENT_TEXT,
        );
        painter.galley(rect.center() - galley.size() * 0.5, galley, ACCENT_TEXT);
    }

    /// Transcribe the first audio file dropped anywhere on the window.
    pub fn handle_dropped_audio(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| {
            i.raw.dropped_files.iter().find_map(|file| {
                let path = file.path.as_ref()?;
                local_stt::is_audio_file(path).then(|| path.clone())
            })
        });
        let Some(path) = dropped else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        match self.dictation_source {
            DictationSource::Provider => {
                if !provider_stt::supports(self.chat_provider) {
                    self.report_error(unsupported_transcription(self.chat_provider));
                    return;
                }
                let api_key = self.api_key();
                if api_key.is_empty() {
                    self.report_error(missing_transcription_key(
                        self.chat_provider,
                        "transcribe audio",
                    ));
                    return;
                }
                let model = provider_stt::file_model(self.chat_provider, &self.chat_model);
                if model.is_empty() {
                    self.report_error("Pick a model in Settings > Provider to transcribe audio.");
                    return;
                }
                let provider = self.chat_provider;
                self.cancel_dictation(ctx);
                self.provider_dictation =
                    Some(ProviderSession::start_file(provider, api_key, model, path));
                self.dictation_stopping = false;
                self.status_message = format!("Transcribing {name}…");
            }
            DictationSource::Local => {
                let Some(paths) = self.dictation_model_paths() else {
                    self.report_error(
                        "Download a dictation model in Settings > Dictation to transcribe audio.",
                    );
                    return;
                };
                self.cancel_dictation(ctx);
                self.local_dictation = Some(LocalSession::start_file(paths, path));
                self.dictation_stopping = false;
                self.status_message = format!("Transcribing {name}…");
            }
        }
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
}
