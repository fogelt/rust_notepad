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
                            egui::vec2(96.0, 30.0),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| self.show_settings_menu(ui),
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

    /// Custom Settings menu: the caret lives inside the button and flips when
    /// the popup is open.
    fn show_settings_menu(&mut self, ui: &mut egui::Ui) {
        let (button_rect, response) = settings_menu_button(ui, self.settings_menu_open);
        if response.clicked() {
            self.settings_menu_open = !self.settings_menu_open;
        }
        if !self.settings_menu_open {
            return;
        }

        let frame = egui::Frame::menu(ui.style());
        let margin = frame.total_margin();
        let position = button_rect.left_bottom() + egui::vec2(-margin.left, 4.0);
        let area = egui::Area::new(egui::Id::new("settings_menu_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(240.0);
                    ui.spacing_mut().slider_width = 80.0;
                    self.show_settings_menu_contents(ui);
                });
            });

        let popup_rect = area.response.rect;
        let pointer = ui.ctx().input(|i| i.pointer.interact_pos());
        let clicked_outside = ui.ctx().input(|i| i.pointer.any_click())
            && pointer.is_some_and(|pos| !popup_rect.contains(pos) && !button_rect.contains(pos));
        if clicked_outside || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_menu_open = false;
        }
    }

    fn show_settings_menu_contents(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Editor").strong());
        ui.horizontal(|ui| {
            ui.label("Font");
            let mut font = self.ui_settings.editor_font;
            ui.radio_value(&mut font, EditorFont::Sans, "Sans");
            ui.radio_value(&mut font, EditorFont::Monospace, "Monospace");
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
        if size.drag_stopped() || (size.changed() && !size.dragged()) {
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
        if line.drag_stopped() || (line.changed() && !line.dragged()) {
            self.save_ui_settings();
        }

        ui.separator();
        ui.label(egui::RichText::new("Preview").strong());
        ui.horizontal(|ui| {
            ui.label("Font");
            let mut font = self.ui_settings.preview_font;
            ui.radio_value(&mut font, EditorFont::Sans, "Sans");
            ui.radio_value(&mut font, EditorFont::Monospace, "Monospace");
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
        if preview.drag_stopped() || (preview.changed() && !preview.dragged()) {
            self.save_ui_settings();
        }
        let preview_line = ui.add(
            egui::Slider::new(
                &mut self.ui_settings.preview_line_spacing,
                UiSettings::PREVIEW_LINE_RANGE,
            )
            .text("Line spacing")
            .suffix(" pt")
            .max_decimals(1)
            .step_by(0.5),
        );
        if preview_line.drag_stopped() || (preview_line.changed() && !preview_line.dragged()) {
            self.save_ui_settings();
        }

        ui.separator();
        if ui.button("Reset appearance").clicked() {
            self.ui_settings = UiSettings::default();
            self.save_ui_settings();
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

/// Settings menu button. The hover/open background fills the whole slot like
/// the File menu, while the label and flipping caret stay tightly grouped.
fn settings_menu_button(ui: &mut egui::Ui, open: bool) -> (egui::Rect, egui::Response) {
    let slot = ui.max_rect();
    let response = ui.interact(slot, ui.id().with("settings_button"), egui::Sense::click());
    let hovered = response.hovered();
    let visuals = if open {
        ui.visuals().widgets.open
    } else if hovered {
        ui.visuals().widgets.hovered
    } else {
        ui.visuals().widgets.inactive
    };

    if ui.is_rect_visible(slot) {
        ui.painter().rect(
            slot,
            visuals.rounding,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
        );

        let font = egui::TextStyle::Button.resolve(ui.style());
        let galley =
            ui.painter()
                .layout_no_wrap(String::from("Settings"), font, egui::Color32::PLACEHOLDER);
        let text_width = galley.size().x;
        let arrow_gap = 9.0;
        let arrow_half_width = 4.0;
        let content_width = text_width + arrow_gap + arrow_half_width * 2.0;
        let start_x = slot.center().x - content_width * 0.5;
        let text_color = visuals.fg_stroke.color;

        ui.painter().galley(
            egui::pos2(start_x, slot.center().y - galley.size().y * 0.5),
            galley,
            text_color,
        );

        let flip =
            ui.ctx()
                .animate_bool_with_time(egui::Id::new("settings_menu_arrow"), open, 0.16);
        draw_chevron(
            ui,
            egui::pos2(
                start_x + text_width + arrow_gap + arrow_half_width,
                slot.center().y,
            ),
            flip,
            if open || hovered { ACCENT } else { text_color },
        );
        if flip > 0.0 && flip < 1.0 {
            ui.ctx().request_repaint();
        }
    }

    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, "Settings"));
    // Make the parent menu bar advance past the whole slot, so its trailing
    // separator sits outside the hover background instead of through it.
    ui.expand_to_include_rect(slot);
    (slot, response)
}

/// Draw a small chevron pointing down at `flip = 0` and up at `flip = 1`.
fn draw_chevron(ui: &egui::Ui, center: egui::Pos2, flip: f32, color: egui::Color32) {
    let angle = std::f32::consts::PI * flip;
    let (sin, cos) = angle.sin_cos();
    let local = [
        egui::vec2(-4.0, -1.5),
        egui::vec2(0.0, 2.5),
        egui::vec2(4.0, -1.5),
    ];
    let rotate = |point: egui::Vec2| {
        egui::pos2(
            center.x + point.x * cos - point.y * sin,
            center.y + point.x * sin + point.y * cos,
        )
    };
    let points = [rotate(local[0]), rotate(local[1]), rotate(local[2])];
    let stroke = egui::Stroke::new(1.6_f32, color);
    ui.painter().line_segment([points[0], points[1]], stroke);
    ui.painter().line_segment([points[1], points[2]], stroke);
}
