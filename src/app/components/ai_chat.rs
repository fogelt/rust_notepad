//! Assistant side panel and report orchestration.
//!
//! Prompt construction, provider transport, and report cleanup live in
//! [`crate::app::ai`]. Dictation state lives in [`crate::app::dictation`]; this
//! module only moves results between channels and app state.

use crate::app::NotepadApp;
use crate::app::ai::provider::Provider;
use crate::app::ai::{prompt, report, text};
use crate::app::theme::{
    ACCENT, ACCENT_TEXT, ASSISTANT_DEFAULT_WIDTH, ASSISTANT_MIN_WIDTH, BG_ELEVATED, BORDER,
    MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH, TEXT, panel_frame, panel_header,
};
use eframe::egui;
use egui_phosphor::regular as icons;
use std::sync::mpsc;
impl NotepadApp {
    fn poll_chat(&mut self) {
        let result = match self.chat_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(mpsc::TryRecvError::Disconnected)) => Some(Err(String::from(
                "The request stopped unexpectedly. Please try again.",
            ))),
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.chat_rx = None;
        match result {
            Ok(text) => {
                self.chat_response = text;
                self.status_message = String::from("Suggestion ready.");
            }
            Err(err) => {
                // Server errors belong in a toast; the suggestion card is for
                // the model's output, not transport failures.
                self.status_message = String::from("The assistant request failed.");
                self.push_toast(
                    text::friendly_generation_error(&err, self.chat_provider),
                    true,
                );
            }
        }
        self.chat_busy = false;
    }

    fn poll_html(&mut self) {
        let result = match self.html_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(mpsc::TryRecvError::Disconnected)) => Some(Err(String::from(
                "The request stopped unexpectedly. Please try again.",
            ))),
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.html_rx = None;
        self.html_busy = false;
        self.html_started = None;
        match result {
            Ok(html) => self.apply_styled_html(html),
            Err(err) => {
                let message = text::friendly_generation_error(&err, self.chat_provider);
                self.status_message = String::from("Could not create the visual report.");
                self.html_error = Some(message.clone());
                self.push_toast(message, true);
            }
        }
    }

    /// Poll recording and transcription for document dictation. Called once
    /// per frame so it also runs while the Assistant panel is closed.
    pub fn poll_dictation(&mut self, ctx: &egui::Context) {
        self.poll_local_dictation(ctx);
        self.poll_provider_dictation(ctx);
        self.poll_whisper_download(ctx);
        self.reveal_dictation(ctx);
        if self.is_dictating() {
            ctx.request_repaint();
        }
    }
    /// Persist the active provider, model, and dictation model.
    pub(crate) fn save_assistant_settings(&mut self) {
        let mut cfg = crate::app::config::load();
        cfg.provider = self.chat_provider.id().to_string();
        cfg.model = self.chat_model.trim().to_string();
        cfg.dictation_model = self.dictation_model.clone();
        cfg.dictation_source = self.dictation_source.id().to_string();
        if let Err(err) = crate::app::config::save(&cfg) {
            self.report_error(err);
        }
    }

    /// API key for the active provider from the OS keychain, falling back to
    /// the provider's environment variable.
    pub(crate) fn api_key(&self) -> String {
        if let Some(key) = crate::app::keys::load(self.chat_provider) {
            return key;
        }
        crate::app::config::pick("", self.chat_provider.env_var())
    }

    /// Send the instruction and the current document to the selected provider.
    fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.is_empty() || self.chat_busy || self.is_dictating() {
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error("No API key: add one in Settings > AI.");
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.report_error("No model selected. Pick one in Settings > AI.");
            return;
        }
        let prompt = prompt::build_prompt(&instruction, &self.content);
        let (tx, rx) = mpsc::channel();
        let provider = self.chat_provider;
        // Reasoning models spend part of the budget before writing, so give
        // them a little more room than Gemini needs.
        let max_tokens = if provider == Provider::Gemini {
            2048
        } else {
            4096
        };
        self.chat_rx = Some(rx);
        self.chat_busy = true;
        self.chat_input.clear();
        self.status_message = format!("Asking {}…", provider.label());
        std::thread::spawn(move || {
            let _ = tx.send(text::generate_text(
                provider, &prompt, &api_key, &model, max_tokens, 0.7,
            ));
        });
    }

    /// Ask the assistant to design a styled HTML version of the document. The
    /// Markdown is left untouched; the HTML is kept in app state and written
    /// to a sidecar file next to the Markdown when possible.
    pub(crate) fn generate_styled_html(&mut self) {
        if self.html_busy || self.chat_busy || self.is_dictating() {
            return;
        }
        if self.content.trim().is_empty() {
            self.report_error("Open or write a document before styling it as HTML.");
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error("No API key: add one in Settings > AI.");
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.report_error("No model selected. Pick one in Settings > AI.");
            return;
        }
        let prompt = prompt::build_html_design_prompt(&self.content);
        let (tx, rx) = mpsc::channel();
        let provider = self.chat_provider;
        let document = self.content.clone();
        let max_tokens = if provider == Provider::Gemini {
            8192
        } else {
            16384
        };
        self.html_rx = Some(rx);
        self.html_busy = true;
        self.html_error = None;
        self.html_started = Some(std::time::Instant::now());
        self.status_message = String::from("Creating a visual report...");
        std::thread::spawn(move || {
            let result = text::generate_text(provider, &prompt, &api_key, &model, max_tokens, 0.4)
                .and_then(|raw| {
                    let html = report::extract_html_document(&raw)?;
                    Ok(report::inject_fallback_image(
                        &report::resolve_commons_images(&html),
                        &document,
                    ))
                });
            let _ = tx.send(result);
        });
    }

    fn apply_styled_html(&mut self, html: String) {
        let html = match report::extract_html_document(&html) {
            Ok(html) => html,
            Err(message) => {
                self.status_message = String::from("Could not create the visual report.");
                self.html_error = Some(message.clone());
                self.html_started = None;
                self.push_toast(message, true);
                return;
            }
        };
        let html = report::mark_layout_tables(&html);
        let html = report::ensure_narrow_screen_support(&html);
        let html = litehtml::html::sanitize_html(&report::ensure_styled_marker(&html));
        self.styled_html = Some(html.clone());
        self.preview_html = true;
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.html_preview = None;
        self.html_error = None;
        self.html_started = None;

        if let Some(path) = self.styled_html_path() {
            match std::fs::write(&path, &html) {
                Ok(()) => {
                    self.status_message = String::from("Visual report ready.");
                }
                Err(_) => {
                    self.report_error("Could not save the visual report.");
                }
            }
        } else {
            self.status_message =
                String::from("Visual report ready. Save the document to keep a copy.");
        }
        self.push_toast("Visual report ready.", false);
    }

    fn styled_html_path(&self) -> Option<std::path::PathBuf> {
        let path = self.file_path.as_ref()?;
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
        {
            return None;
        }
        Some(path.with_extension("html"))
    }

    /// Load a styled HTML sidecar saved next to the current Markdown file.
    pub(crate) fn load_styled_html_sidecar(&mut self) {
        self.styled_html = None;
        self.html_preview = None;
        self.html_error = None;
        if let Some(path) = self.styled_html_path()
            && let Ok(html) = std::fs::read_to_string(path)
            && html.contains(report::STYLED_HTML_MARKER)
        {
            // Older sidecars may lack the narrow-screen rules; add them in
            // memory without rewriting the saved file.
            let html = report::ensure_narrow_screen_support(&report::mark_layout_tables(&html));
            self.styled_html = Some(litehtml::html::sanitize_html(&html));
        }
        self.preview_html = self.styled_html.is_some();
        self.preview_revision = self.preview_revision.wrapping_add(1);
    }

    pub fn show_chat_panel(&mut self, ctx: &egui::Context) {
        // Poll even when the Assistant panel is closed: visual reports can be
        // started from the toolbar while the user is in the editor/preview.
        self.poll_chat();
        self.poll_html();
        if self.chat_busy || self.html_busy {
            ctx.request_repaint();
        }
        if !self.tabs.assistant {
            return;
        }

        let has_key = self.has_api_key(self.chat_provider);
        let send_shortcut = ctx.format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Enter,
        ));

        egui::SidePanel::left("ai_chat_panel")
            .resizable(true)
            .default_width(ASSISTANT_DEFAULT_WIDTH)
            .min_width(ASSISTANT_MIN_WIDTH)
            // Cap the width so the panel can never be dragged wide enough
            // to hide the central editor entirely.
            .max_width((ctx.available_rect().width() - MIN_EDITOR_WIDTH).max(MIN_SIDE_PANEL_WIDTH))
            .frame(panel_frame(ctx))
            .show(ctx, |ui| {
                if panel_header(ui, "Assistant", "Close").clicked() {
                    self.tabs.assistant = false;
                }
                ui.separator();
                // Roomier spacing, and everything below scrolls so short
                // windows clip nothing instead of hiding content.
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                let total_height = ui.available_height();
                let top_max = (total_height - 190.0).max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(top_max)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Instruction").strong());
                            if !self.chat_input.is_empty() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Clear").clicked() {
                                            self.chat_input.clear();
                                        }
                                    },
                                );
                            }
                        });

                        let input = egui::TextEdit::multiline(&mut self.chat_input)
                            .hint_text("e.g. Turn these bullets into a table…")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        // Ctrl+Enter while typing sends. Checked against the raw
                        // input state (not `lost_focus`, which never fires here
                        // because Enter keeps focus in a multiline field).
                        if input.response.has_focus()
                            && ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)
                        {
                            self.send_chat();
                        }

                        let can_send =
                            !self.chat_busy && !self.chat_input.trim().is_empty();

                        ui.horizontal_wrapped(|ui| {
                            let (send_text, send_color, send_fill) = if self.chat_busy {
                                // The leading spaces reserve room for the
                                // spinner that replaces the plane icon.
                                (
                                    String::from("       Thinking…"),
                                    TEXT,
                                    ui.visuals().widgets.inactive.weak_bg_fill,
                                )
                            } else {
                                (
                                    format!("{} Send  ({send_shortcut})", icons::PAPER_PLANE_TILT),
                                    ACCENT_TEXT,
                                    ACCENT,
                                )
                            };
                            let send = egui::Button::new(
                                egui::RichText::new(&send_text)
                                    .strong()
                                    .color(send_color),
                            )
                            .fill(send_fill);
                            let response = ui
                                .add_enabled(can_send, send)
                                .on_hover_text("Send the instruction to Gemini");

                            if self.chat_busy {
                                // A bare spinner sits where the plane icon was.
                                let galley = egui::WidgetText::from(
                                    egui::RichText::new(&send_text)
                                        .strong()
                                        .color(send_color),
                                )
                                .into_galley(
                                    ui,
                                    Some(false),
                                    f32::INFINITY,
                                    egui::TextStyle::Button,
                                );
                                let text_rect = ui.layout().align_size_within_rect(
                                    galley.size(),
                                    response.rect.shrink2(ui.spacing().button_padding),
                                );
                                let spinner_center = egui::pos2(
                                    text_rect.left() + 7.0,
                                    response.rect.center().y,
                                );
                                egui::Spinner::new().size(16.0).color(ACCENT).paint_at(
                                    ui,
                                    egui::Rect::from_center_size(
                                        spinner_center,
                                        egui::vec2(16.0, 16.0),
                                    ),
                                );
                            }

                            if response.clicked() {
                                self.send_chat();
                            }
                        });

                        if !has_key {
                            ui.weak("Add an API key in Settings > AI to use the assistant.");
                        } else {
                            ui.weak(
                                "Use the microphone in the editor toolbar to dictate into the document.",
                            );
                        }

                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Suggestion").strong());
                    if !self.chat_response.is_empty() {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.weak(
                                    egui::RichText::new(
                                        self.chat_provider.model_label(&self.chat_model),
                                    )
                                    .small(),
                                );
                            },
                        );
                    }
                });
                ui.add_space(4.0);

                egui::Frame::none()
                    .fill(BG_ELEVATED)
                    .rounding(8.0)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        let inner_height = ui.available_height();
                        ui.set_min_height(inner_height);
                        if self.chat_response.is_empty() {
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), ui.available_height()),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.add_space(
                                        ((ui.available_height() - 46.0) * 0.5).max(8.0),
                                    );
                                    ui.label(
                                        egui::RichText::new(icons::SPARKLE)
                                            .size(22.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Gemini's suggestion will appear here.",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                },
                            );
                        } else {
                            // Reserve the action row and a small gap, then let
                            // the response take the rest. The fixed math keeps
                            // the card inside the panel at every height.
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let action_height = 30.0;
                            let gap = 6.0;
                            let body_height = (inner_height - action_height - gap).max(0.0);

                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), body_height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false; 2])
                                        .max_height(body_height)
                                        .show(ui, |ui| {
                                            ui.add_sized(
                                                egui::vec2(
                                                    ui.available_width(),
                                                    body_height,
                                                ),
                                                egui::TextEdit::multiline(
                                                    &mut self.chat_response,
                                                )
                                                .desired_width(f32::INFINITY)
                                                .frame(false)
                                                .code_editor(),
                                            );
                                        });
                                },
                            );
                            ui.add_space(gap);

                            let dictating = self.is_dictating();
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Insert"))
                                    .on_hover_text("Insert at cursor")
                                    .clicked()
                                {
                                    let snippet = self.chat_response.clone();
                                    crate::app::components::editor::insert_at_cursor(self, ctx, &snippet);
                                    self.note_content_changed();
                                }
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Replace"))
                                    .on_hover_text("Replace document")
                                    .clicked()
                                {
                                    self.content = self.chat_response.clone();
                                    self.note_content_changed();
                                }
                                if ui.button("Clear").clicked() {
                                    self.chat_response.clear();
                                }
                            });
                        }
                    });

            });
    }
}
