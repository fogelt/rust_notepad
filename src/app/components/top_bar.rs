use crate::app::NotepadApp;
use crate::app::ai::provider::Provider;
use crate::app::config::{EditorFont, UiSettings};
use crate::app::local_stt;
use crate::app::models::{DictationSource, SettingsTab};
use crate::app::provider_stt;
use crate::app::theme::{
    ACCENT, ACCENT_HOVERED, ACCENT_TEXT, BG_ELEVATED, BORDER_STRONG, CHEVRON_ANIMATION_TIME,
    SURFACE, SURFACE_ACTIVE, SURFACE_HOVERED, draw_chevron,
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

/// Segmented settings tab, styled like the view tabs in the header.
fn settings_tab_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    selected: bool,
    width: f32,
    rounding: egui::Rounding,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = response.hovered();
    let fill = if selected {
        if hovered { ACCENT_HOVERED } else { ACCENT }
    } else if hovered {
        SURFACE_ACTIVE
    } else {
        SURFACE_HOVERED
    };
    ui.painter().rect_filled(rect, rounding, fill);
    let color = if selected {
        ACCENT_TEXT
    } else if hovered {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{icon} {label}"),
        egui::FontId::proportional(14.0),
        color,
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, selected, label));
    response.clicked()
}

/// Frameless icon button used inside settings tables.
fn table_icon_button(ui: &mut egui::Ui, icon: &str, hover: &str, enabled: bool) -> bool {
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::new(egui::RichText::new(icon).size(14.0)).frame(false),
        )
        .on_hover_text(hover);
    let response = if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    response.clicked()
}

/// Fixed-width column cell so table rows line up across the full width.
fn table_cell(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 20.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            add(ui);
        },
    )
    .response
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
        self.styled_html = None;
        self.html_preview = None;
        self.html_error = None;
        self.preview_html = false;
        self.preview_revision = self.preview_revision.wrapping_add(1);
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
                self.load_styled_html_sidecar();
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
                            egui::vec2(84.0, 30.0),
                            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                ui.menu_button(format!("{} File", icons::FILE), |ui| {
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
                            egui::vec2(116.0, 30.0),
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
        let width = match self.settings_tab {
            SettingsTab::Appearance => 260.0,
            SettingsTab::Provider | SettingsTab::Dictation => 470.0,
        };
        let area = egui::Area::new(egui::Id::new("settings_menu_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().slider_width = 80.0;
                    egui::ScrollArea::vertical()
                        .max_height(620.0)
                        .show(ui, |ui| self.show_settings_menu_contents(ui));
                });
            });

        let popup_rect = area.response.rect;
        let pointer = ui.ctx().input(|i| i.pointer.interact_pos());
        // ComboBox popups are separate areas; don't treat their clicks as
        // outside clicks.
        let another_popup_open = ui.ctx().memory(|m| m.any_popup_open());
        let clicked_outside = !another_popup_open
            && ui.ctx().input(|i| i.pointer.any_click())
            && pointer.is_some_and(|pos| !popup_rect.contains(pos) && !button_rect.contains(pos));
        if clicked_outside || ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_menu_open = false;
        }
    }

    fn show_settings_menu_contents(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(SURFACE)
            .rounding(9.0)
            .inner_margin(egui::Margin::same(2.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal(|ui| {
                    if settings_tab_button(
                        ui,
                        icons::PALETTE,
                        "Appearance",
                        self.settings_tab == SettingsTab::Appearance,
                        128.0,
                        egui::Rounding {
                            nw: 7.0,
                            sw: 7.0,
                            ne: 0.0,
                            se: 0.0,
                        },
                    ) {
                        self.settings_tab = SettingsTab::Appearance;
                    }
                    if settings_tab_button(
                        ui,
                        icons::PLUG,
                        "Provider",
                        self.settings_tab == SettingsTab::Provider,
                        112.0,
                        egui::Rounding::ZERO,
                    ) {
                        self.settings_tab = SettingsTab::Provider;
                    }
                    if settings_tab_button(
                        ui,
                        icons::MICROPHONE,
                        "Dictation",
                        self.settings_tab == SettingsTab::Dictation,
                        120.0,
                        egui::Rounding {
                            nw: 0.0,
                            sw: 0.0,
                            ne: 7.0,
                            se: 7.0,
                        },
                    ) {
                        self.settings_tab = SettingsTab::Dictation;
                    }
                });
            });
        ui.separator();
        match self.settings_tab {
            SettingsTab::Appearance => self.show_appearance_settings(ui),
            SettingsTab::Provider => self.show_provider_settings(ui),
            SettingsTab::Dictation => self.show_dictation_settings(ui),
        }
    }

    fn show_appearance_settings(&mut self, ui: &mut egui::Ui) {
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

    fn show_provider_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Choose provider").strong());
        ui.weak(
            "Click a provider to use it. Keys are stored in the system keychain and never \
             shown again.",
        );
        let rows: Vec<_> = Provider::ALL
            .iter()
            .map(|provider| {
                (
                    *provider,
                    self.stored_keys.contains(provider),
                    self.has_api_key(*provider),
                )
            })
            .collect();
        let table_padding = 6.0;
        let cell_gap = 8.0;
        let content_width = (ui.available_width() - table_padding * 2.0).max(220.0);
        let provider_w = content_width * 0.38;
        let status_w = content_width * 0.32;
        let actions_w = (content_width - provider_w - status_w - cell_gap * 2.0).max(72.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = cell_gap;
            ui.add_space(table_padding);
            table_cell(ui, provider_w, |ui| {
                ui.label(egui::RichText::new("Provider").small().strong());
            });
            table_cell(ui, status_w, |ui| {
                ui.label(egui::RichText::new("Status").small().strong());
            });
            table_cell(ui, actions_w, |ui| {
                ui.label(egui::RichText::new("Actions").small().strong());
            });
        });
        ui.add_space(2.0);

        for (row_index, (provider, stored, available)) in rows.into_iter().enumerate() {
            let active = provider == self.chat_provider;
            let mut actions_rect = egui::Rect::NOTHING;
            let stripe = if row_index % 2 == 1 {
                SURFACE
            } else {
                egui::Color32::TRANSPARENT
            };
            let row = egui::Frame::none()
                .fill(stripe)
                .inner_margin(egui::Margin::symmetric(table_padding, 4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = cell_gap;
                        table_cell(ui, provider_w, |ui| {
                            if active {
                                ui.label(
                                    egui::RichText::new(provider.label()).color(ACCENT).strong(),
                                );
                            } else {
                                ui.label(provider.label());
                            }
                        });
                        table_cell(ui, status_w, |ui| {
                            if stored {
                                ui.label(egui::RichText::new("Saved").small().color(ACCENT));
                            } else if available {
                                ui.label(egui::RichText::new("From environment").small().weak());
                            } else {
                                ui.label(egui::RichText::new("Not set").small().weak());
                            }
                        });
                        let actions = table_cell(ui, actions_w, |ui| {
                            ui.horizontal(|ui| {
                                if table_icon_button(
                                    ui,
                                    icons::ARROW_SQUARE_OUT,
                                    "Get an API key",
                                    true,
                                ) {
                                    let _ = webbrowser::open(provider.key_url());
                                }
                                if stored {
                                    if table_icon_button(
                                        ui,
                                        icons::PENCIL_SIMPLE,
                                        "Replace key",
                                        true,
                                    ) {
                                        self.key_edit = Some(provider);
                                        self.key_input.clear();
                                    }
                                    if table_icon_button(ui, icons::TRASH, "Remove key", true) {
                                        match crate::app::keys::delete(provider) {
                                            Ok(()) => {
                                                self.stored_keys.remove(&provider);
                                                self.key_error = None;
                                                self.status_message = format!(
                                                    "{} API key removed.",
                                                    provider.label()
                                                );
                                            }
                                            Err(err) => self.key_error = Some(err),
                                        }
                                    }
                                } else if table_icon_button(ui, icons::KEY, "Add key", true) {
                                    self.key_edit = Some(provider);
                                    self.key_input.clear();
                                }
                            });
                        });
                        actions_rect = actions.rect;
                    });
                });
            if active {
                ui.painter().rect_stroke(
                    row.response.rect,
                    6.0,
                    egui::Stroke::new(1.0_f32, ACCENT),
                );
            }
            let pointer_over_actions = ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|pos| actions_rect.contains(pos));
            if !pointer_over_actions {
                let response = ui.interact(
                    row.response.rect,
                    ui.id().with(("provider_row", provider.id())),
                    egui::Sense::click(),
                );
                if !active && response.hovered() {
                    ui.painter().rect_stroke(
                        row.response.rect,
                        6.0,
                        egui::Stroke::new(1.0_f32, BORDER_STRONG),
                    );
                }
                let response = response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("Use {}", provider.label()));
                if response.clicked() && !active {
                    self.chat_provider = provider;
                    if !provider.has_model(&self.chat_model) {
                        self.chat_model = provider.default_model().to_string();
                    }
                    self.key_edit = None;
                    self.key_input.clear();
                    self.save_assistant_settings();
                }
            }
        }

        if let Some(provider) = self.key_edit {
            ui.add_space(4.0);
            ui.label(format!("{} API key", provider.label()));
            let mut save = false;
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.key_input)
                        .password(true)
                        .hint_text(provider.key_hint())
                        .desired_width(f32::INFINITY),
                );
                save = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    self.key_edit = None;
                    self.key_input.clear();
                }
            });
            if save {
                let key = self.key_input.trim().to_string();
                if !key.is_empty() {
                    match crate::app::keys::store(provider, &key) {
                        Ok(()) => {
                            self.stored_keys.insert(provider);
                            self.key_edit = None;
                            self.key_input.clear();
                            self.key_error = None;
                            self.status_message = format!("{} API key saved.", provider.label());
                        }
                        Err(err) => self.key_error = Some(err),
                    }
                }
            }
        }
        if let Some(err) = &self.key_error {
            ui.colored_label(ui.visuals().error_fg_color, err);
        }

        ui.separator();
        ui.label(egui::RichText::new("Model").strong());
        let models = self.chat_provider.models();
        if models.is_empty() {
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.chat_model)
                        .hint_text("Model ID, e.g. openai/gpt-4o-mini")
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                self.save_assistant_settings();
            }
        } else {
            let current = self.chat_model.clone();
            let selected = self.chat_provider.model_label(&self.chat_model);
            egui::ComboBox::from_id_source("settings_model")
                .width(ui.available_width())
                .selected_text(selected)
                .icon(|ui, rect, visuals, is_open, _| {
                    let flip = ui.ctx().animate_bool_with_time(
                        egui::Id::new("settings_model_chevron"),
                        is_open,
                        CHEVRON_ANIMATION_TIME,
                    );
                    let color = if is_open || ui.rect_contains_pointer(rect) {
                        ACCENT
                    } else {
                        visuals.fg_stroke.color
                    };
                    draw_chevron(ui, rect.center(), flip, color);
                })
                .show_ui(ui, |ui| {
                    for model in models {
                        ui.selectable_value(
                            &mut self.chat_model,
                            model.id.to_string(),
                            format!("{} · {}", model.label, model.note),
                        );
                    }
                    if !self.chat_provider.has_model(&current) && !current.is_empty() {
                        ui.separator();
                        ui.selectable_value(
                            &mut self.chat_model,
                            current.clone(),
                            format!("{current} (saved)"),
                        );
                    }
                });
            if self.chat_model != current {
                self.save_assistant_settings();
            }
        }
    }

    fn show_dictation_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Dictation").strong());
        ui.weak("Choose whether speech is transcribed by your provider or by a local model.");

        egui::Frame::none()
            .fill(SURFACE)
            .rounding(9.0)
            .inner_margin(egui::Margin::same(2.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal(|ui| {
                    if settings_tab_button(
                        ui,
                        icons::CLOUD,
                        "Provider",
                        self.dictation_source == DictationSource::Provider,
                        116.0,
                        egui::Rounding {
                            nw: 7.0,
                            sw: 7.0,
                            ne: 0.0,
                            se: 0.0,
                        },
                    ) {
                        self.dictation_source = DictationSource::Provider;
                        self.save_assistant_settings();
                    }
                    if settings_tab_button(
                        ui,
                        icons::HARD_DRIVE,
                        "Local",
                        self.dictation_source == DictationSource::Local,
                        86.0,
                        egui::Rounding {
                            nw: 0.0,
                            sw: 0.0,
                            ne: 7.0,
                            se: 7.0,
                        },
                    ) {
                        self.dictation_source = DictationSource::Local;
                        self.save_assistant_settings();
                    }
                });
            });

        ui.add_space(4.0);
        match self.dictation_source {
            DictationSource::Provider => {
                if provider_stt::supports(self.chat_provider) {
                    let model = provider_stt::transcription_model(self.chat_provider);
                    ui.weak(format!(
                        "Uses {} · {model}. The local models below are disabled.",
                        self.chat_provider.label()
                    ));
                    if !self.has_api_key(self.chat_provider) {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            format!(
                                "Add a {} API key in Provider settings to use it.",
                                self.chat_provider.label()
                            ),
                        );
                    }
                } else {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!(
                            "{} doesn't offer transcription. Switch to Gemini or OpenAI in \
                             Provider settings, or choose Local.",
                            self.chat_provider.label()
                        ),
                    );
                }
                ui.add_enabled_ui(false, |ui| self.show_local_models(ui));
            }
            DictationSource::Local => {
                ui.weak(
                    "Whisper runs on this computer. Sizes are one-time downloads; RAM use is \
                     approximate while transcribing.",
                );
                self.show_local_models(ui);
            }
        }
    }

    fn show_local_models(&mut self, ui: &mut egui::Ui) {
        let table_padding = 6.0;
        let cell_gap = 6.0;
        let content_width = (ui.available_width() - table_padding * 2.0).max(260.0);
        let model_w = content_width * 0.34;
        let size_w = content_width * 0.13;
        let ram_w = content_width * 0.12;
        let status_w = content_width * 0.19;
        let actions_w =
            (content_width - model_w - size_w - ram_w - status_w - cell_gap * 4.0).max(48.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = cell_gap;
            ui.add_space(table_padding);
            table_cell(ui, model_w, |ui| {
                ui.label(egui::RichText::new("Model").small().strong());
            });
            table_cell(ui, size_w, |ui| {
                ui.label(egui::RichText::new("Size").small().strong());
            });
            table_cell(ui, ram_w, |ui| {
                ui.label(egui::RichText::new("RAM").small().strong());
            });
            table_cell(ui, status_w, |ui| {
                ui.label(egui::RichText::new("Status").small().strong());
            });
            table_cell(ui, actions_w, |ui| {
                ui.label(egui::RichText::new("Actions").small().strong());
            });
        });
        ui.add_space(2.0);

        for (row_index, model) in local_stt::MODELS.iter().enumerate() {
            let installed = local_stt::model_paths(model).is_some();
            let active = self.dictation_model == model.id;
            let downloading = self
                .whisper_download
                .as_ref()
                .is_some_and(|download| download.model.id == model.id);
            let any_download = self.whisper_download.is_some();
            let mut actions_rect = egui::Rect::NOTHING;
            let stripe = if row_index % 2 == 1 {
                SURFACE
            } else {
                egui::Color32::TRANSPARENT
            };
            let row = egui::Frame::none()
                .fill(stripe)
                .inner_margin(egui::Margin::symmetric(table_padding, 4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = cell_gap;
                        table_cell(ui, model_w, |ui| {
                            let label = if active {
                                egui::RichText::new(model.label).color(ACCENT).strong()
                            } else {
                                egui::RichText::new(model.label)
                            };
                            ui.label(label).on_hover_text(model.note);
                        });
                        table_cell(ui, size_w, |ui| {
                            ui.label(
                                egui::RichText::new(format!("~{}", local_stt::archive_size(model)))
                                    .small(),
                            );
                        });
                        table_cell(ui, ram_w, |ui| {
                            ui.label(egui::RichText::new(model.ram).small());
                        });
                        table_cell(ui, status_w, |ui| {
                            if installed {
                                ui.label(egui::RichText::new("Installed").small().color(ACCENT));
                            } else if downloading {
                                ui.label(egui::RichText::new("Downloading").small().weak());
                            } else {
                                ui.label(egui::RichText::new("Not downloaded").small().weak());
                            }
                        });
                        let actions = table_cell(ui, actions_w, |ui| {
                            ui.horizontal(|ui| {
                                if installed {
                                    if table_icon_button(
                                        ui,
                                        icons::TRASH,
                                        "Remove the model from this computer",
                                        !self.is_dictating(),
                                    ) {
                                        match local_stt::remove_model(model) {
                                            Ok(()) => {
                                                self.whisper_error = None;
                                                if self.dictation_model == model.id {
                                                    let fallback = local_stt::installed_models()
                                                        .first()
                                                        .copied()
                                                        .map(|model| model.id)
                                                        .unwrap_or(local_stt::DEFAULT_MODEL);
                                                    self.dictation_model = fallback.to_string();
                                                    self.save_assistant_settings();
                                                }
                                                self.status_message =
                                                    String::from("Dictation model removed.");
                                            }
                                            Err(err) => self.whisper_error = Some(err),
                                        }
                                    }
                                } else if downloading {
                                    ui.spinner();
                                } else {
                                    let enabled = !any_download;
                                    if table_icon_button(
                                        ui,
                                        icons::DOWNLOAD_SIMPLE,
                                        if any_download {
                                            "Wait for the current download to finish"
                                        } else {
                                            "Download this model"
                                        },
                                        enabled,
                                    ) {
                                        self.whisper_error = None;
                                        self.whisper_download =
                                            Some(local_stt::ModelDownload::start(model));
                                        self.status_message =
                                            format!("Downloading {}…", model.label);
                                    }
                                }
                            });
                        });
                        actions_rect = actions.rect;
                    });
                });

            if active {
                ui.painter().rect_stroke(
                    row.response.rect,
                    6.0,
                    egui::Stroke::new(1.0_f32, ACCENT),
                );
            }
            let pointer_over_actions = ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|pos| actions_rect.contains(pos));
            if installed && !pointer_over_actions {
                let response = ui.interact(
                    row.response.rect,
                    ui.id().with(("dictation_row", model.id)),
                    egui::Sense::click(),
                );
                if !active && response.hovered() {
                    ui.painter().rect_stroke(
                        row.response.rect,
                        6.0,
                        egui::Stroke::new(1.0_f32, BORDER_STRONG),
                    );
                }
                let response = response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("Use {}", model.label));
                if response.clicked() && !active {
                    self.dictation_model = model.id.to_string();
                    self.save_assistant_settings();
                }
            }
        }

        if let Some(download) = &self.whisper_download {
            ui.add_space(4.0);
            ui.add(
                egui::ProgressBar::new(download.fraction())
                    .desired_width(ui.available_width())
                    .desired_height(6.0)
                    .rounding(3.0)
                    .fill(ACCENT),
            );
            ui.weak(format!(
                "{} · {} of {}",
                download.model.label,
                format_megabytes(download.downloaded),
                format_megabytes(download.total)
            ));
        }
        if let Some(err) = &self.whisper_error {
            ui.colored_label(ui.visuals().error_fg_color, err);
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

fn format_megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
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
        let galley = ui.painter().layout_no_wrap(
            format!("{} Settings", icons::GEAR),
            font,
            egui::Color32::PLACEHOLDER,
        );
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

        let flip = ui.ctx().animate_bool_with_time(
            egui::Id::new("settings_menu_arrow"),
            open,
            CHEVRON_ANIMATION_TIME,
        );
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
