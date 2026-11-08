use crate::app::NotepadApp;
use crate::app::theme::ACCENT_TEXT;
use eframe::egui;
use egui_phosphor::regular as icons;
use rfd::FileDialog;
use std::fs;
use std::path::Path;

const FILE_FILTERS: [(&str, &[&str]); 3] = [
    ("Markdown", &["md", "markdown"]),
    ("Text", &["txt"]),
    ("All files", &["*"]),
];

/// Header tab with a min-width so the title always fits; click toggles a panel.
/// Same 24px box and Button text metrics as the File button so all
/// labels sit on one line.
fn view_tab(ui: &mut egui::Ui, selected: &mut bool, label: &str, hover: &str) {
    let text = if *selected {
        egui::RichText::new(label).color(ACCENT_TEXT)
    } else {
        egui::RichText::new(label)
    };
    if ui
        .add_sized(
            egui::vec2(88.0, 26.0),
            egui::Button::new(text).selected(*selected).rounding(6.0),
        )
        .on_hover_text(hover)
        .clicked()
    {
        *selected = !*selected;
    }
}

fn with_filters(dialog: FileDialog) -> FileDialog {
    let mut dialog = dialog;
    for (name, extensions) in FILE_FILTERS {
        dialog = dialog.add_filter(name, extensions);
    }
    dialog
}

impl NotepadApp {
    fn new_file(&mut self) {
        self.content.clear();
        self.file_path = None;
        self.markdown_cache = Default::default();
        self.refresh_stats();
        self.status_message = String::from("New file created.");
    }

    fn open_file(&mut self) {
        let Some(path) = with_filters(FileDialog::new()).pick_file() else {
            return;
        };
        match fs::read_to_string(&path) {
            Ok(text) => {
                self.content = text;
                self.file_path = Some(path);
                self.markdown_cache = Default::default();
                self.refresh_stats();
                self.status_message = String::from("Opened file successfully.");
            }
            Err(err) => {
                self.status_message = format!("Could not open file: {err}");
            }
        }
    }

    /// Save to the existing path, or prompt when there is none / `save_as`.
    fn save_file(&mut self, save_as: bool) {
        let mut path = self.file_path.clone();
        if save_as || path.is_none() {
            let mut dialog = with_filters(FileDialog::new());
            if let Some(current) = &self.file_path {
                dialog = dialog.set_directory(
                    current
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(".")),
                );
                if let Some(name) = current.file_name() {
                    dialog = dialog.set_file_name(name.to_string_lossy());
                }
            } else {
                dialog = dialog.set_file_name("untitled.md");
            }
            let Some(picked) = dialog.save_file() else {
                return;
            };
            path = Some(picked);
            self.file_path = path.clone();
        }

        if let Some(path) = path {
            match fs::write(&path, &self.content) {
                Ok(()) => {
                    self.status_message = format!("Saved {}", display_name(&path));
                }
                Err(err) => {
                    self.status_message = format!("Failed to save file: {err}");
                }
            }
        }
    }

    pub fn show_top_bar(&mut self, ctx: &egui::Context) {
        let new_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N);
        let open_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
        let save_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
        let save_as_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::S,
        );

        let mut action_new = false;
        let mut action_open = false;
        let mut action_save = false;
        let mut action_save_as = false;

        ctx.input_mut(|input| {
            // Check Save As first: its shortcut also holds Ctrl/Cmd.
            if input.consume_shortcut(&save_as_shortcut) {
                action_save_as = true;
            } else if input.consume_shortcut(&save_shortcut) {
                action_save = true;
            }
            if input.consume_shortcut(&new_shortcut) {
                action_new = true;
            }
            if input.consume_shortcut(&open_shortcut) {
                action_open = true;
            }
        });

        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::none()
                    .fill(crate::app::theme::BG_ELEVATED)
                    .inner_margin(egui::Margin::symmetric(8.0, 5.0)),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    // Centered so the shorter File button aligns with
                    // the fixed-height tabs instead of riding high.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        // Fixed box like the tabs so the labels share one
                        // baseline.
                        ui.allocate_ui_with_layout(
                            egui::vec2(52.0, 26.0),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                ui.menu_button("File", |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(format!("{} New", icons::FILE_PLUS))
                                                .shortcut_text(ctx.format_shortcut(&new_shortcut)),
                                        )
                                        .clicked()
                                    {
                                        action_new = true;
                                        ui.close_menu();
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new(format!(
                                                "{} Open…",
                                                icons::FOLDER_OPEN
                                            ))
                                            .shortcut_text(ctx.format_shortcut(&open_shortcut)),
                                        )
                                        .clicked()
                                    {
                                        action_open = true;
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui
                                        .add(
                                            egui::Button::new(format!(
                                                "{} Save",
                                                icons::FLOPPY_DISK
                                            ))
                                            .shortcut_text(ctx.format_shortcut(&save_shortcut)),
                                        )
                                        .clicked()
                                    {
                                        action_save = true;
                                        ui.close_menu();
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new(format!(
                                                "{} Save As…",
                                                icons::FLOPPY_DISK
                                            ))
                                            .shortcut_text(ctx.format_shortcut(&save_as_shortcut)),
                                        )
                                        .clicked()
                                    {
                                        action_save_as = true;
                                        ui.close_menu();
                                    }
                                });
                            },
                        );
                        ui.separator();
                        // Order mirrors the panel layout: assistant (left),
                        // editor (center), preview (right).
                        view_tab(
                            ui,
                            &mut self.tabs.assistant,
                            "Assistant",
                            "Toggle the Gemini assistant panel",
                        );
                        view_tab(
                            ui,
                            &mut self.tabs.editor,
                            "Editor",
                            "Toggle the editor panel",
                        );
                        view_tab(
                            ui,
                            &mut self.tabs.preview,
                            "Preview",
                            "Toggle the Markdown preview panel (F11 for fullscreen)",
                        );
                    });
                });
            });

        if action_new {
            self.new_file();
        }
        if action_open {
            self.open_file();
        }
        if action_save_as {
            self.save_file(true);
        } else if action_save {
            self.save_file(false);
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
