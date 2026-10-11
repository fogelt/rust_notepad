use egui_commonmark::CommonMarkCache;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::app::components::ai_chat::ChatOutcome;

pub struct NotepadApp {
    pub content: String,
    pub file_path: Option<PathBuf>,
    pub status_message: String,
    pub markdown_cache: CommonMarkCache,
    pub chat_open: bool,
    pub chat_input: String,
    pub chat_response: String,
    pub chat_error: Option<String>,
    pub chat_busy: bool,
    pub chat_rx: Option<Receiver<ChatOutcome>>,
    pub chat_model: String,
    pub chat_models: Vec<String>,
    pub chat_models_loading: bool,
    pub chat_models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    pub chat_models_error: Option<String>,
    /// Successful Gemini requests this session, per model id.
    pub chat_session_requests: HashMap<String, u32>,
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
        Self {
            content: String::new(),
            file_path: None,
            status_message: String::from("Ready"),
            markdown_cache: CommonMarkCache::default(),
            chat_open: false,
            chat_input: String::new(),
            chat_response: String::new(),
            chat_error: None,
            chat_busy: false,
            chat_rx: None,
            chat_model: std::env::var("GEMINI_MODEL")
                .map(|m| m.trim().to_string())
                .unwrap_or_default(),
            chat_models: Vec::new(),
            chat_models_loading: false,
            chat_models_rx: None,
            chat_models_error: None,
            chat_session_requests: HashMap::new(),
        }
    }
}
