pub struct NotepadApp {
    pub content: String,
    pub filename: String,
    pub status_message: String,
}

impl Default for NotepadApp {
    fn default() -> Self {
        Self {
            content: String::new(),
            filename: String::from("notes.txt"),
            status_message: String::from("Ready"),
        }
    }
}
