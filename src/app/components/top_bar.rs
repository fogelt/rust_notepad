use crate::app::NotepadApp;
use crate::app::config::{EditorFont, UiSettings};
use crate::app::theme::{
    ACCENT, ACCENT_HOVERED, ACCENT_TEXT, BG_ELEVATED, SURFACE, SURFACE_ACTIVE, SURFACE_HOVERED,
};
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

/// Segmented view tab: selected tabs get the accent fill, the rest stay
/// transparent inside the shared pill container and light up on hover.
/// Outer corners are rounded; edges shared with a neighbouring tab are square.
fn view_tab(
    ui: &mut egui::Ui,
    selected: &mut bool,
    label: &str,
    hover: &str,
    rounding: egui::Rounding,
) -> egui::Rect {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(92.0, 26.0), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = response.hovered();

    let fill = if *selected {
        if hovered { ACCENT_HOVERED } else { ACCENT }
    } else if hovered {
        SURFACE_ACTIVE
    } else {
        SURFACE_HOVERED
    };
    ui.painter().rect_filled(rect, rounding, fill);

    let text_color = if *selected {
        ACCENT_TEXT
    } else if hovered {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        text_color,
    );

    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, *selected, label));
    if response.clicked() {
        *selected = !*selected;
    }
    response.on_hover_text(hover);
    rect
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
                        let available_width = ui.available_width();
                        let tabs_width = 3.0 * 92.0 + 4.0;
                        let start_x = ui.cursor().min.x;
                        // Fixed box like the tabs so the labels share one
                        // baseline.
                        ui.allocate_ui_with_layout(
                            egui::vec2(56.0, 30.0),
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
                        ui.allocate_ui_with_layout(
                            egui::vec2(104.0, 30.0),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    let response = ui
                                        .menu_button("Settings", |ui| {
                                            ui.set_min_width(250.0);

                                            ui.label(egui::RichText::new("Editor").strong());
                                            ui.horizontal(|ui| {
                                                ui.label("Font");
                                                let mut font =
                                                    self.ui_settings.editor_font;
                                                ui.radio_value(
                                                    &mut font,
                                                    EditorFont::Sans,
                                                    "Sans",
                                                );
                                                ui.radio_value(
                                                    &mut font,
                                                    EditorFont::Monospace,
                                                    "Monospace",
                                                );
                                                if font != self.ui_settings.editor_font {
                                                    self.ui_settings.editor_font = font;
                                                    self.save_ui_settings();
                                                }
                                            });

                                            let size = ui.add(
                                                egui::Slider::new(
                                                    &mut self.ui_settings.editor_font_size,
                                                    UiSettings::EDITOR_SIZE_RANGE,
                                                )
                                                .text("Text size")
                                                .suffix(" pt")
                                                .step_by(0.5),
                                            );
                                            if size.drag_stopped()
                                                || (size.changed() && !size.dragged())
                                            {
                                                self.save_ui_settings();
                                            }
                                            let line = ui.add(
                                                egui::Slider::new(
                                                    &mut self.ui_settings.editor_line_height,
                                                    UiSettings::EDITOR_LINE_RANGE,
                                                )
                                                .text("Line spacing")
                                                .suffix(" pt")
                                                .step_by(1.0),
                                            );
                                            if line.drag_stopped()
                                                || (line.changed() && !line.dragged())
                                            {
                                                self.save_ui_settings();
                                            }

                                            ui.separator();
                                            ui.label(egui::RichText::new("Preview").strong());
                                            ui.horizontal(|ui| {
                                                ui.label("Font");
                                                let mut font =
                                                    self.ui_settings.preview_font;
                                                ui.radio_value(
                                                    &mut font,
                                                    EditorFont::Sans,
                                                    "Sans",
                                                );
                                                ui.radio_value(
                                                    &mut font,
                                                    EditorFont::Monospace,
                                                    "Monospace",
                                                );
                                                if font != self.ui_settings.preview_font {
                                                    self.ui_settings.preview_font = font;
                                                    self.save_ui_settings();
                                                }
                                            });
                                            let preview = ui.add(
                                                egui::Slider::new(
                                                    &mut self.ui_settings.preview_font_size,
                                                    UiSettings::PREVIEW_SIZE_RANGE,
                                                )
                                                .text("Text size")
                                                .suffix(" pt")
                                                .step_by(0.5),
                                            );
                                            if preview.drag_stopped()
                                                || (preview.changed() && !preview.dragged())
                                            {
                                                self.save_ui_settings();
                                            }

                                            ui.separator();
                                            if ui.button("Reset appearance").clicked() {
                                                self.ui_settings = UiSettings::default();
                                                self.save_ui_settings();
                                            }
                                            ui.weak("Saved automatically.");
                                        })
                                        .response;
                                    menu_caret(ui, &response);
                                });
                            },
                        );
                        ui.separator();
                        // Center the segmented tabs in the full bar, not just
                        // in the space remaining after the File menu.
                        let consumed = ui.cursor().min.x - start_x;
                        let tabs_left = ((available_width - tabs_width) * 0.5).max(consumed);
                        ui.add_space(tabs_left - consumed);
                        // A single segmented container keeps the view tabs
                        // grouped and lets the active panel carry the accent.
                        egui::Frame::none()
                            .fill(SURFACE)
                            .rounding(9.0)
                            .inner_margin(egui::Margin::same(2.0))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.horizontal(|ui| {
                                    // Order mirrors the panel layout: assistant
                                    // (left), editor (center), preview (right).
                                    let tab_rects = [
                                        view_tab(
                                            ui,
                                            &mut self.tabs.assistant,
                                            "Assistant",
                                            "Toggle the Gemini assistant panel",
                                            egui::Rounding {
                                                nw: 7.0,
                                                sw: 7.0,
                                                ne: 0.0,
                                                se: 0.0,
                                            },
                                        ),
                                        view_tab(
                                            ui,
                                            &mut self.tabs.editor,
                                            "Editor",
                                            "Toggle the editor panel",
                                            egui::Rounding::ZERO,
                                        ),
                                        view_tab(
                                            ui,
                                            &mut self.tabs.preview,
                                            "Preview",
                                            "Toggle the Markdown preview panel (F11 for fullscreen)",
                                            egui::Rounding {
                                                nw: 0.0,
                                                sw: 0.0,
                                                ne: 7.0,
                                                se: 7.0,
                                            },
                                        ),
                                    ];

                                    // Hairlines between neighbouring tabs,
                                    // drawn last so they also sit above the
                                    // accent fill of selected tabs.
                                    for rect in tab_rects.iter().skip(1) {
                                        ui.painter().vline(
                                            rect.left(),
                                            egui::Rangef::new(
                                                rect.top() + 6.0,
                                                rect.bottom() - 6.0,
                                            ),
                                            egui::Stroke::new(1.0_f32, BG_ELEVATED),
                                        );
                                    }
                                });
                            });
                    });
                });
            });

        if action_new {
            self.cancel_dictation(ctx);
            self.new_file();
        }
        if action_open {
            self.cancel_dictation(ctx);
            self.open_file();
        }
        if action_save_as {
            self.save_file(true);
        } else if action_save {
            self.save_file(false);
        }
    }

    /// Persist appearance settings without touching the assistant credentials.
    fn save_ui_settings(&mut self) {
        let mut cfg = crate::app::config::load();
        cfg.ui = self.ui_settings.clone();
        if let Err(err) = crate::app::config::save(&cfg) {
            self.status_message = format!("Could not save settings: {err}");
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Downward caret beside the Settings menu button. It slides down and
/// brightens on hover so the dropdown affordance is obvious.
fn menu_caret(ui: &mut egui::Ui, button: &egui::Response) {
    let t = ui.ctx().animate_bool_with_time(
        egui::Id::new("settings_menu_caret"),
        button.hovered(),
        0.15,
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 20.0), egui::Sense::hover());
    let color = mix_color(ui.visuals().weak_text_color(), ACCENT, t);
    ui.painter().text(
        rect.center() + egui::vec2(0.0, 2.0 * t),
        egui::Align2::CENTER_CENTER,
        icons::CARET_DOWN,
        egui::FontId::proportional(11.0 + 1.5 * t),
        color,
    );
    if t > 0.0 && t < 1.0 {
        ui.ctx().request_repaint();
    }
}

fn mix_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}
