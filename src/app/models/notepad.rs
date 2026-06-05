use std::fs;
use std::path::PathBuf;

pub struct NotepadApp {
    pub content: String,
    pub file_path: Option<PathBuf>,
    pub status_message: String,
}

impl NotepadApp {
    pub fn with_file(path: Option<PathBuf>) -> Self {
        if let Some(p) = path {
            match fs::read_to_string(&p) {
                Ok(text) => Self {
                    content: text,
                    status_message: format!("Opened: {:?}", p.file_name().unwrap()),
                    file_path: Some(p),
                },
                Err(e) => Self {
                    content: String::new(),
                    status_message: format!("Failed to auto-load file: {}", e),
                    file_path: None,
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
        }
    }
}
