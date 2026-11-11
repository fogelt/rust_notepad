use eframe::egui;
use egui_commonmark::CommonMarkCache;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::app::config::UiSettings;

/// Events streamed from the dictation worker.
pub enum DictationEvent {
    /// A new piece of transcription text to append to the document.
    Delta(String),
    /// A speculative partial transcription that replaces the previous one.
    Interim(String),
    /// An authoritative transcription for the current speech segment.
    Final(String),
    /// The worker finished the transcription.
    Finished,
    /// The request failed; the payload is the raw error.
    Failed(String),
}

/// An in-progress streamed dictation. Kept so the document can be updated as
/// text arrives and restored exactly if the request is cancelled.
pub struct DictationStream {
    pub original_content: String,
    pub original_range: Option<(usize, usize)>,
    /// Byte offset where streamed text is appended.
    pub start_b: usize,
    /// Text already inserted into the document (without the trailing space).
    pub inserted: String,
    /// Text received that hasn't been revealed yet.
    pub pending: String,
    /// Current interim transcription shown after the committed text.
    pub interim: String,
    /// Whether a separating space must be added after the stream ends.
    pub after_needs_space: bool,
    /// True once the worker finished; the pending text still needs revealing.
    pub finished: bool,
}

/// Transient overlay message, used for user-friendly errors.
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

/// Page shown in the top-bar Settings menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Provider,
    Dictation,
}

/// Whether dictation uses a cloud provider or the local model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DictationSource {
    #[default]
    Provider,
    Local,
}

impl DictationSource {
    pub fn id(self) -> &'static str {
        match self {
            DictationSource::Provider => "provider",
            DictationSource::Local => "local",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "provider" | "cloud" => Some(DictationSource::Provider),
            "local" | "offline" => Some(DictationSource::Local),
            _ => None,
        }
    }
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
    /// Render the preview through the HTML engine instead of CommonMark.
    pub preview_html: bool,
    pub html_preview: Option<crate::app::components::html_preview::HtmlPreview>,
    /// Bumped when the preview mode/source changes to reset its scroll offset.
    pub preview_revision: u64,
    pub chat_input: String,
    pub chat_response: String,
    pub chat_busy: bool,
    pub chat_rx: Option<Receiver<Result<String, String>>>,
    /// Assistant-generated styled HTML for the current document, kept separate
    /// from the Markdown so the source is never modified.
    pub styled_html: Option<String>,
    pub html_busy: bool,
    pub html_rx: Option<Receiver<Result<String, String>>>,
    pub html_error: Option<String>,
    /// When the current visual-report request started, for the elapsed timer.
    pub html_started: Option<Instant>,
    pub chat_model: String,
    /// Provider used for assistant text and visual reports.
    pub chat_provider: crate::app::components::provider::Provider,
    /// Providers with a key saved in the OS credential store.
    pub stored_keys: std::collections::HashSet<crate::app::components::provider::Provider>,
    /// Provider whose key is currently being entered.
    pub key_edit: Option<crate::app::components::provider::Provider>,
    /// Transient input for the key being entered; cleared after saving.
    pub key_input: String,
    /// Last keychain error, shown in Settings.
    pub key_error: Option<String>,
    /// Page shown in the top-bar Settings menu.
    pub settings_tab: SettingsTab,
    /// Local Whisper model used for dictation.
    pub dictation_model: String,
    /// Cloud provider or local Whisper for dictation.
    pub dictation_source: DictationSource,
    /// Local Whisper model download in progress.
    pub whisper_download: Option<crate::app::local_stt::ModelDownload>,
    /// Last local model download error, shown in settings.
    pub whisper_error: Option<String>,
    /// Active offline dictation session.
    pub local_dictation: Option<crate::app::local_stt::LocalSession>,
    /// Active provider dictation session.
    pub provider_dictation: Option<crate::app::provider_stt::ProviderSession>,
    /// True once the local session is flushing its last audio.
    pub dictation_stopping: bool,
    /// Document edit being filled in by the stream.
    pub dictation_stream: Option<DictationStream>,
    /// Toast shown above the footer; auto-dismisses.
    pub toast: Option<Toast>,
    /// Persisted appearance settings (font family, sizes, line spacing).
    pub ui_settings: UiSettings,
    /// Whether the Settings menu popup is open.
    pub settings_menu_open: bool,
    /// Last seen monitor size / DPI, to detect screen changes.
    pub last_monitor_size: Option<[f32; 2]>,
    pub last_native_ppp: Option<f32>,
    /// Title currently set on the OS window (avoids re-sending it every frame).
    pub window_title: Option<String>,
    /// Whether the startup window has been resized for this monitor.
    pub initial_window_sized: bool,
    /// Cached app mark used in the empty editor/preview states.
    pub logo_texture: Option<egui::TextureHandle>,
    /// Cached document statistics for the footer (refreshed on edits).
    pub word_count: usize,
    pub char_count: usize,
}

impl NotepadApp {
    /// True while the microphone is recording.
    pub fn is_dictating(&self) -> bool {
        self.local_dictation.is_some() || self.provider_dictation.is_some()
    }

    /// Whether a provider has a key in the keychain or its environment.
    pub fn has_api_key(&self, provider: crate::app::components::provider::Provider) -> bool {
        self.stored_keys.contains(&provider)
            || !crate::app::config::pick("", provider.env_var()).is_empty()
    }

    /// The selected local dictation model.
    pub fn dictation_model(&self) -> &'static crate::app::local_stt::WhisperModel {
        crate::app::local_stt::model(&self.dictation_model)
    }

    /// Paths for the selected model, if it is fully downloaded.
    pub fn dictation_model_paths(&self) -> Option<crate::app::local_stt::ModelPaths> {
        crate::app::local_stt::model_paths(self.dictation_model())
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
        let mut app = if let Some(p) = path {
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
        };
        app.load_styled_html_sidecar();
        app
    }
}

impl Default for NotepadApp {
    fn default() -> Self {
        // Stored settings win; environment (.env) is the fallback.
        let stored = crate::app::config::load();
        use crate::app::components::provider::Provider;
        // An explicit provider wins; otherwise guess from the stored key.
        let provider = Provider::from_id(&stored.provider)
            .or_else(|| Provider::detect(&stored.api_key))
            .unwrap_or_default();
        let dictation_model = crate::app::local_stt::resolve_model(&stored.dictation_model);
        let dictation_source =
            DictationSource::from_id(&stored.dictation_source).unwrap_or_else(|| {
                if crate::app::provider_stt::supports(provider) {
                    DictationSource::Provider
                } else {
                    DictationSource::Local
                }
            });
        let model = if !stored.model.trim().is_empty() {
            stored.model.trim().to_string()
        } else {
            let env_model = if provider == Provider::Gemini {
                crate::app::config::pick("", "GEMINI_MODEL")
            } else {
                String::new()
            };
            if env_model.is_empty() {
                provider.default_model().to_string()
            } else {
                env_model
            }
        };
        // Migration: move a legacy plaintext key into the OS keychain, then
        // rewrite the config without it. This runs once.
        let mut key_error = None;
        if !stored.api_key.trim().is_empty() {
            let key_provider = Provider::from_id(&stored.provider)
                .or_else(|| Provider::detect(&stored.api_key))
                .unwrap_or_default();
            if let Err(err) = crate::app::keys::store(key_provider, &stored.api_key) {
                key_error = Some(err);
            }
            let mut cleared = stored.clone();
            cleared.api_key.clear();
            let _ = crate::app::config::save(&cleared);
        }
        // Only credentials in the OS keychain count as "saved"; environment
        // keys are reported separately in Settings.
        let stored_keys: std::collections::HashSet<Provider> = Provider::ALL
            .into_iter()
            .filter(|provider| crate::app::keys::has(*provider))
            .collect();
        Self {
            content: String::new(),
            file_path: None,
            status_message: String::from("Ready"),
            markdown_cache: CommonMarkCache::default(),
            tabs: PanelTabs::default(),
            preview_html: false,
            html_preview: None,
            preview_revision: 0,
            chat_input: String::new(),
            chat_response: String::new(),
            chat_busy: false,
            chat_rx: None,
            styled_html: None,
            html_busy: false,
            html_rx: None,
            html_error: None,
            html_started: None,
            chat_model: model,
            chat_provider: provider,
            stored_keys,
            key_edit: None,
            key_input: String::new(),
            key_error,
            settings_tab: SettingsTab::default(),
            dictation_model: dictation_model.id.to_string(),
            dictation_source,
            whisper_download: None,
            whisper_error: None,
            local_dictation: None,
            provider_dictation: None,
            dictation_stopping: false,
            dictation_stream: None,
            toast: None,
            ui_settings: stored.ui.clone(),
            settings_menu_open: false,
            last_monitor_size: None,
            last_native_ppp: None,
            window_title: None,
            initial_window_sized: false,
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
