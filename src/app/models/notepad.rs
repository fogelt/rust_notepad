use eframe::egui;
use egui_commonmark::CommonMarkCache;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::app::audio::RecordingSession;

/// Events streamed from the dictation worker.
pub enum DictationEvent {
    /// A new piece of transcription text to append to the document.
    Delta(String),
    /// Gemini finished the transcription.
    Finished,
    /// The request failed; the payload is the raw error.
    Failed(String),
}

/// An in-progress streamed dictation. Kept so the document can be updated as
/// text arrives and restored exactly if the request fails.
pub struct DictationStream {
    pub original_content: String,
    pub original_range: Option<(usize, usize)>,
    /// Byte offset where streamed text is appended.
    pub start_b: usize,
    /// Text already inserted into the document (without the trailing space).
    pub inserted: String,
    /// Text received from Gemini that hasn't been revealed yet.
    pub pending: String,
    /// Whether a separating space must be added after the stream ends.
    pub after_needs_space: bool,
    /// True once Gemini finished; the pending text still needs revealing.
    pub finished: bool,
}

/// Transient overlay message, used for user-friendly Gemini errors.
pub struct Toast {
    pub message: String,
    pub is_error: bool,
    pub created: Instant,
}

/// Visibility toggles for the three main panels, driven by the header tabs.
/// Adding a new panel: add a field here, a tab in `show_top_bar`, and gate
/// the panel in its `show_*` function.
pub struct PanelTabs {
    pub editor: bool,
    pub preview: bool,
    pub assistant: bool,
}

impl Default for PanelTabs {
    fn default() -> Self {
        Self {
            editor: true,
            preview: true,
            assistant: false,
        }
    }
}

pub struct NotepadApp {
    pub content: String,
    pub file_path: Option<PathBuf>,
    pub status_message: String,
    pub markdown_cache: CommonMarkCache,
    pub tabs: PanelTabs,
    pub chat_input: String,
    pub chat_response: String,
    pub chat_error: Option<String>,
    pub chat_busy: bool,
    pub chat_rx: Option<Receiver<Result<String, String>>>,
    pub chat_model: String,
    pub chat_api_key: String,
    /// Show the API key in plaintext (transient UI state, not persisted).
    pub chat_show_key: bool,
    /// Microphone recording in progress (document dictation).
    pub recording: Option<RecordingSession>,
    /// True while a transcription request is in flight.
    pub transcribing: bool,
    pub transcribe_rx: Option<Receiver<DictationEvent>>,
    /// Set to ask the streaming worker to stop early.
    pub transcribe_cancel: Option<Arc<AtomicBool>>,
    /// Document edit being filled in by the stream.
    pub dictation_stream: Option<DictationStream>,
    /// Toast shown above the footer; auto-dismisses.
    pub toast: Option<Toast>,
    /// Last seen monitor size / DPI, to detect screen changes.
    pub last_monitor_size: Option<[f32; 2]>,
    pub last_native_ppp: Option<f32>,
    /// Title currently set on the OS window (avoids re-sending it every frame).
    pub window_title: Option<String>,
    /// Cached app mark used in the empty editor/preview states.
    pub logo_texture: Option<egui::TextureHandle>,
    /// Cached document statistics for the footer (refreshed on edits).
    pub word_count: usize,
    pub char_count: usize,
}

impl NotepadApp {
    /// True while the microphone is recording or Gemini is transcribing.
    pub fn is_dictating(&self) -> bool {
        self.recording.is_some() || self.transcribing
    }

    /// Recompute the cached footer statistics after `content` changes.
    pub fn refresh_stats(&mut self) {
        self.word_count = self.content.split_whitespace().count();
        self.char_count = self.content.chars().count();
    }

    /// Call after a programmatic edit (toolbar or assistant). Autosaves when
    /// the document has a path; otherwise reports unsaved changes.
    pub fn note_content_changed(&mut self) {
        self.refresh_stats();
        if let Some(path) = &self.file_path {
            match std::fs::write(path, &self.content) {
                Ok(()) => self.status_message = String::from("Autosaved changes."),
                Err(err) => self.status_message = format!("Autosave failed: {err}"),
            }
        } else {
            self.status_message = String::from("Unsaved changes (Save file to enable autosave).");
        }
    }

    pub fn with_file(path: Option<PathBuf>) -> Self {
        if let Some(p) = path {
            match std::fs::read_to_string(&p) {
                Ok(text) => {
                    let word_count = text.split_whitespace().count();
                    let char_count = text.chars().count();
                    Self {
                        content: text,
                        status_message: format!("Opened: {:?}", p.file_name().unwrap()),
                        file_path: Some(p),
                        word_count,
                        char_count,
                        ..Default::default()
                    }
                }
                // A missing file means "new note at this path": keep the path
                // so the first Save writes where the user asked (e.g. `rmd notes.md`).
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self {
                    content: String::new(),
                    status_message: format!(
                        "New file: {:?} (Save to create it)",
                        p.file_name().unwrap_or(p.as_os_str())
                    ),
                    file_path: Some(p),
                    ..Default::default()
                },
                Err(e) => Self {
                    status_message: format!("Failed to auto-load file: {}", e),
                    ..Default::default()
                },
            }
        } else {
            Self::default()
        }
    }
}

impl Default for NotepadApp {
    fn default() -> Self {
        // Stored settings win; environment (.env) is the fallback.
        let stored = crate::app::config::load();
        Self {
            content: String::new(),
            file_path: None,
            status_message: String::from("Ready"),
            markdown_cache: CommonMarkCache::default(),
            tabs: PanelTabs::default(),
            chat_input: String::new(),
            chat_response: String::new(),
            chat_error: None,
            chat_busy: false,
            chat_rx: None,
            chat_model: {
                let model = crate::app::config::pick(&stored.model, "GEMINI_MODEL");
                if model.is_empty() {
                    crate::app::components::ai_chat::default_model().to_string()
                } else {
                    model
                }
            },
            chat_api_key: crate::app::config::pick(&stored.api_key, "GEMINI_API_KEY"),
            chat_show_key: false,
            recording: None,
            transcribing: false,
            transcribe_rx: None,
            transcribe_cancel: None,
            dictation_stream: None,
            toast: None,
            last_monitor_size: None,
            last_native_ppp: None,
            window_title: None,
            logo_texture: None,
            word_count: 0,
            char_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_default_to_editor_and_preview() {
        let app = NotepadApp::default();
        assert!(app.tabs.editor);
        assert!(app.tabs.preview);
        assert!(!app.tabs.assistant);
    }

    #[test]
    fn missing_file_becomes_new_buffer_at_that_path() {
        let dir = std::env::temp_dir().join(format!(
            "rust_notepad_missing_file_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let app = NotepadApp::with_file(Some(dir.join("notes.md")));
        assert_eq!(app.content, "");
        assert_eq!(app.file_path, Some(dir.join("notes.md")));
        assert!(app.status_message.contains("New file"));
    }

    #[test]
    fn existing_file_is_loaded() {
        let dir = std::env::temp_dir().join(format!(
            "rust_notepad_existing_file_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("existing.md");
        std::fs::write(&path, "# hi").unwrap();
        let app = NotepadApp::with_file(Some(path.clone()));
        assert_eq!(app.content, "# hi");
        assert_eq!(app.file_path, Some(path));
        assert_eq!(app.word_count, 2);
        assert_eq!(app.char_count, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
