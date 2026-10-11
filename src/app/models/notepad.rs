use egui_commonmark::CommonMarkCache;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

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
    pub chat_rx: Option<Receiver<Result<String, String>>>,
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
        }
    }
}
