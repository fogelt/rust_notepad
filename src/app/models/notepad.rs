use egui_commonmark::CommonMarkCache;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::app::components::ai_chat::ChatOutcome;

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
    pub chat_rx: Option<Receiver<ChatOutcome>>,
    pub chat_model: String,
    pub chat_api_key: String,
    /// Show the API key in plaintext (transient UI state, not persisted).
    pub chat_show_key: bool,
    pub chat_models: Vec<String>,
    pub chat_models_loading: bool,
    pub chat_models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    pub chat_models_error: Option<String>,
    /// Successful Gemini requests this session, per model id.
    pub chat_session_requests: HashMap<String, u32>,
    /// Last seen monitor size / DPI, to detect screen changes.
    pub last_monitor_size: Option<[f32; 2]>,
    pub last_native_ppp: Option<f32>,
}

impl NotepadApp {
    pub fn with_file(path: Option<PathBuf>) -> Self {
        if let Some(p) = path {
            match std::fs::read_to_string(&p) {
                Ok(text) => Self {
                    content: text,
                    status_message: format!("Opened: {:?}", p.file_name().unwrap()),
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
            chat_model: crate::app::config::pick(&stored.model, "GEMINI_MODEL"),
            chat_api_key: crate::app::config::pick(&stored.api_key, "GEMINI_API_KEY"),
            chat_show_key: false,
            chat_models: Vec::new(),
            chat_models_loading: false,
            chat_models_rx: None,
            chat_models_error: None,
            chat_session_requests: HashMap::new(),
            last_monitor_size: None,
            last_native_ppp: None,
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
}
