use crate::app::NotepadApp;
use crate::app::theme::{
    ACCENT, ASSISTANT_DEFAULT_WIDTH, ASSISTANT_MIN_WIDTH, MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH,
    panel_frame, panel_header,
};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui_phosphor::regular as icons;
use std::sync::mpsc;

/// Outcome of one background Gemini request, tagged with the model used.
pub struct ChatOutcome {
    pub model: String,
    pub result: Result<String, String>,
}

/// Free-tier rate limit snapshot (peak use over 28d / limit).
/// Source: AI Studio dashboard, last updated October 11 2026.
/// Google exposes no quota/usage endpoint for API keys, so these values are
/// static; check the AI Studio dashboard for live numbers.
pub struct RateLimit {
    pub id: &'static str,
    pub display: &'static str,
    pub category: &'static str,
    pub peak_rpm: u64,
    pub rpm: u64,
    pub peak_tpm: u64,
    pub tpm: u64,
    pub peak_rpd: u64,
    pub rpd: u64,
}

pub const RATE_LIMITS: &[RateLimit] = &[
    RateLimit {
        id: "gemini-2.5-flash",
        display: "Gemini 2.5 Flash",
        category: "Text-out models",
        peak_rpm: 3,
        rpm: 5,
        peak_tpm: 201,
        tpm: 250_000,
        peak_rpd: 20,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3.1-flash-lite",
        display: "Gemini 3.1 Flash Lite",
        category: "Text-out models",
        peak_rpm: 2,
        rpm: 15,
        peak_tpm: 632,
        tpm: 250_000,
        peak_rpd: 8,
        rpd: 500,
    },
    RateLimit {
        id: "gemini-2.5-flash-lite",
        display: "Gemini 2.5 Flash Lite",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 10,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3-flash",
        display: "Gemini 3 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 5,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3.5-flash",
        display: "Gemini 3.5 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 5,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3.5-flash-lite",
        display: "Gemini 3.5 Flash Lite",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 15,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 500,
    },
    RateLimit {
        id: "gemini-3.6-flash",
        display: "Gemini 3.6 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 5,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3.7-flash",
        display: "Gemini 3.7 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 5,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-3.8-flash",
        display: "Gemini 3.8 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 5,
        peak_tpm: 0,
        tpm: 250_000,
        peak_rpd: 0,
        rpd: 20,
    },
    RateLimit {
        id: "gemini-2.0-flash",
        display: "Gemini 2 Flash",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 0,
        peak_tpm: 0,
        tpm: 0,
        peak_rpd: 0,
        rpd: 0,
    },
    RateLimit {
        id: "gemini-2.0-flash-lite",
        display: "Gemini 2 Flash Lite",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 0,
        peak_tpm: 0,
        tpm: 0,
        peak_rpd: 0,
        rpd: 0,
    },
    RateLimit {
        id: "gemini-2.5-pro",
        display: "Gemini 2.5 Pro",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 0,
        peak_tpm: 0,
        tpm: 0,
        peak_rpd: 0,
        rpd: 0,
    },
    RateLimit {
        id: "gemini-3.1-pro",
        display: "Gemini 3.1 Pro",
        category: "Text-out models",
        peak_rpm: 0,
        rpm: 0,
        peak_tpm: 0,
        tpm: 0,
        peak_rpd: 0,
        rpd: 0,
    },
];

/// Model ids with a non-zero free-tier limit, used when the live list
/// cannot be fetched.
pub fn fallback_models() -> Vec<String> {
    RATE_LIMITS
        .iter()
        .filter(|l| l.rpm > 0 || l.rpd > 0)
        .map(|l| l.id.to_string())
        .collect()
}

fn normalize_model_id(s: &str) -> String {
    s.strip_prefix("models/")
        .unwrap_or(s)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Find the static limit entry for a model id (exact match first, then the
/// longest entry id that is a prefix, to catch dated variants like
/// `gemini-2.5-flash-preview-05-20`).
pub fn find_limits(model: &str) -> Option<&'static RateLimit> {
    let norm = normalize_model_id(model);
    if let Some(exact) = RATE_LIMITS
        .iter()
        .find(|l| normalize_model_id(l.id) == norm)
    {
        return Some(exact);
    }
    RATE_LIMITS
        .iter()
        .filter(|l| norm.starts_with(&normalize_model_id(l.id)))
        .max_by_key(|l| l.id.len())
}

pub fn fmt_num(n: u64) -> String {
    if n >= 1000 && n.is_multiple_of(1000) {
        format!("{}K", n / 1000)
    } else {
        n.to_string()
    }
}

/// List models available to this API key via `GET v1beta/models`,
/// keeping only those that support `generateContent`.
fn fetch_models(api_key: &str) -> Result<Vec<String>, String> {
    let resp = reqwest::blocking::Client::new()
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .query(&[("key", api_key)])
        .send()
        .map_err(|e| format!("Could not list models: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Could not list models: {}", resp.status()));
    }
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Could not parse model list: {e}"))?;
    let mut models = json["models"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?;
            let methods = m["supportedGenerationMethods"].as_array();
            let supports =
                methods.is_some_and(|ms| ms.iter().any(|v| v.as_str() == Some("generateContent")));
            if supports {
                Some(name.strip_prefix("models/").unwrap_or(name).to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    models.sort();
    if models.is_empty() {
        return Err("No generateContent models returned for this key.".to_string());
    }
    Ok(models)
}

/// Build the prompt sent to Gemini: user instruction + current document.
fn build_prompt(instruction: &str, document: &str) -> String {
    format!(
        "You are helping edit a Markdown document in a notepad app.\n\
         Follow the user's instruction below. Return ONLY the resulting Markdown \
         (no code fences, no commentary) unless the user explicitly asks for an explanation.\n\n\
         --- USER INSTRUCTION ---\n\
         {instruction}\n\n\
         --- CURRENT DOCUMENT ---\n\
         {document}"
    )
}

/// Strip a wrapping ```markdown fence if the model added one anyway.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the first line (`markdown`, `md`).
        let mut lines = inner.lines();
        let first = lines.next().unwrap_or("");
        let first_trim = first.trim();
        if first_trim.eq_ignore_ascii_case("markdown") || first_trim.eq_ignore_ascii_case("md") {
            return lines.collect::<Vec<_>>().join("\n").trim().to_string();
        }
        return inner.trim().to_string();
    }
    trimmed.to_string()
}

fn gemini_generate(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.7, "maxOutputTokens": 2048 }
    });
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .query(&[("key", api_key)])
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("Gemini error {status}: {short}"));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Bad response from Gemini: {e}"))?;
    json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(strip_code_fence)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Gemini returned an empty response.".to_string())
}

fn insert_at_cursor(app: &mut NotepadApp, ctx: &egui::Context, snippet: &str) {
    let id = egui::Id::new("editor_text");
    if let Some(state) = TextEditState::load(ctx, id)
        && let Some(range) = state.cursor.char_range()
    {
        let idx = range.primary.index.max(range.secondary.index);
        let byte_pos = app
            .content
            .char_indices()
            .nth(idx)
            .map(|(b, _)| b)
            .unwrap_or(app.content.len());
        let mut to_insert = snippet.to_string();
        if !to_insert.ends_with('\n') {
            to_insert.push('\n');
        }
        app.content.insert_str(byte_pos, &to_insert);
        let new_c = app.content[..byte_pos + to_insert.len()].chars().count();
        if let Some(mut st) = TextEditState::load(ctx, id) {
            st.cursor
                .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(new_c))));
            st.store(ctx, id);
        }
        ctx.memory_mut(|m| m.request_focus(id));
        return;
    }
    if !app.content.is_empty() && !app.content.ends_with('\n') {
        app.content.push('\n');
    }
    app.content.push_str(snippet);
    if !app.content.ends_with('\n') {
        app.content.push('\n');
    }
}

impl NotepadApp {
    fn poll_chat(&mut self) {
        if let Some(rx) = self.chat_rx.as_ref()
            && let Ok(outcome) = rx.try_recv()
        {
            match outcome.result {
                Ok(text) => {
                    *self.chat_session_requests.entry(outcome.model).or_insert(0) += 1;
                    self.chat_response = text;
                    self.chat_error = None;
                    self.status_message = String::from("Gemini suggestion ready.");
                }
                Err(err) => {
                    self.chat_error = Some(err);
                    self.status_message = String::from("Gemini request failed.");
                }
            }
            self.chat_rx = None;
            self.chat_busy = false;
        }
    }

    fn poll_models(&mut self) {
        if let Some(rx) = self.chat_models_rx.as_ref()
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(models) => {
                    if self.chat_model.is_empty() || !models.iter().any(|m| m == &self.chat_model) {
                        // Keep a usable selection: prefer the previous
                        // default, otherwise the first live model.
                        if let Some(first) = models.first() {
                            self.chat_model = first.clone();
                        }
                    }
                    self.chat_models = models;
                    self.chat_models_error = None;
                }
                Err(err) => {
                    self.chat_models = fallback_models();
                    if self.chat_model.is_empty()
                        && let Some(first) = self.chat_models.first()
                    {
                        self.chat_model = first.clone();
                    }
                    self.chat_models_error = Some(err);
                }
            }
            self.chat_models_rx = None;
            self.chat_models_loading = false;
        }
    }

    /// API key from the settings field, falling back to the environment.
    fn api_key(&self) -> String {
        if !self.chat_api_key.trim().is_empty() {
            return self.chat_api_key.trim().to_string();
        }
        crate::app::config::pick("", "GEMINI_API_KEY")
    }

    /// Fetch the live model list once (needs the API key). Falls back to
    /// the static snapshot when the request fails.
    fn ensure_models_loaded(&mut self) {
        if !self.chat_models.is_empty() || self.chat_models_loading || self.chat_models_rx.is_some()
        {
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.chat_models_rx = Some(rx);
        self.chat_models_loading = true;
        std::thread::spawn(move || {
            let _ = tx.send(fetch_models(&api_key));
        });
    }

    fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.is_empty() || self.chat_busy {
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.chat_error =
                Some("No API key: paste one into the API key field above.".to_string());
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.chat_error =
                Some("No model selected. Pick one from the dropdown above.".to_string());
            return;
        }
        let prompt = build_prompt(&instruction, &self.content);
        let (tx, rx) = mpsc::channel();
        self.chat_rx = Some(rx);
        self.chat_busy = true;
        self.chat_error = None;
        self.chat_input.clear();
        self.status_message = String::from("Asking Gemini...");
        std::thread::spawn(move || {
            let result = gemini_generate(&prompt, &api_key, &model);
            let _ = tx.send(ChatOutcome { model, result });
        });
    }

    fn show_rate_limits(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Rate limits")
            .default_open(true)
            .show(ui, |ui| {
                let Some(lim) = find_limits(&self.chat_model) else {
                    ui.weak("No limit data for this model.");
                    ui.weak("Snapshot Oct 11 2026 · free tier.");
                    return;
                };
                if lim.rpm == 0 && lim.rpd == 0 && lim.tpm == 0 {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        "Free-tier limit is 0 — this model looks unavailable \
                     (requests fail with 429 limit: 0). Pick another model.",
                    );
                }
                ui.label(format!("{} · {}", lim.display, lim.category));
                ui.label(format!(
                    "RPM  {} / {}   ·   TPM  {} / {}   ·   RPD  {} / {}",
                    fmt_num(lim.peak_rpm),
                    fmt_num(lim.rpm),
                    fmt_num(lim.peak_tpm),
                    fmt_num(lim.tpm),
                    fmt_num(lim.peak_rpd),
                    fmt_num(lim.rpd),
                ));
                ui.weak("Peak use over 28 days vs limit.");
                // Session progress against the daily (RPD) limit. Google offers
                // no per-key usage endpoint, so this counts this app session only.
                let used = self
                    .chat_session_requests
                    .get(&self.chat_model)
                    .copied()
                    .unwrap_or(0);
                if lim.rpd > 0 {
                    let frac = (used as f32 / lim.rpd as f32).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .fill(ACCENT)
                            .rounding(egui::Rounding::ZERO)
                            // Note: no `.show_percentage()` — it replaces
                            // custom text (and its color), so the percent
                            // is baked into our own white label instead.
                            .text(
                                egui::RichText::new(format!(
                                    "{used}/{} · {}%",
                                    lim.rpd,
                                    (frac * 100.0).round() as u32,
                                ))
                                .color(egui::Color32::WHITE),
                            ),
                    );
                } else {
                    ui.weak(format!("{used} session requests."));
                }
                ui.weak(
                    "Limits snapshot Oct 11 2026 · free tier · live usage: AI Studio dashboard.",
                );
            });
    }

    pub fn show_chat_panel(&mut self, ctx: &egui::Context) {
        if !self.tabs.assistant {
            return;
        }
        self.poll_chat();
        self.ensure_models_loaded();
        self.poll_models();
        if self.chat_busy || self.chat_models_loading {
            ctx.request_repaint();
        }

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
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label("API key:");
                            // Eye toggle lives inside the field, on the left;
                            // the widened text margin keeps typed text clear.
                            let mut text_margin = egui::Margin::symmetric(4.0, 2.0);
                            text_margin.left = 30.0;
                            let field = ui.add(
                                egui::TextEdit::singleline(&mut self.chat_api_key)
                                    .password(!self.chat_show_key)
                                    .hint_text("Paste your Gemini API key…")
                                    .desired_width(f32::INFINITY)
                                    .margin(text_margin),
                            );
                            let eye_rect = egui::Rect::from_min_size(
                                field.rect.left_top()
                                    + egui::vec2(5.0, (field.rect.height() - 20.0) / 2.0),
                                egui::vec2(20.0, 20.0),
                            );
                            let eye = if self.chat_show_key {
                                icons::EYE
                            } else {
                                icons::EYE_SLASH
                            };
                            if ui
                                .put(
                                    eye_rect,
                                    egui::Button::new(egui::RichText::new(eye).size(15.0))
                                        .frame(false),
                                )
                                .on_hover_text(if self.chat_show_key {
                                    "Hide the key"
                                } else {
                                    "Show the key"
                                })
                                .clicked()
                            {
                                self.chat_show_key = !self.chat_show_key;
                            }
                        });
                        if !self.chat_api_key.trim().is_empty()
                            && !self.chat_api_key.trim().starts_with("AIza")
                        {
                            ui.weak("Keys usually start with “AIza” — check for extra spaces.");
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Save key & model").clicked() {
                                let cfg = crate::app::config::AssistantConfig {
                                    api_key: self.chat_api_key.trim().to_string(),
                                    model: self.chat_model.trim().to_string(),
                                };
                                match crate::app::config::save(&cfg) {
                                    Ok(path) => {
                                        self.chat_api_key = cfg.api_key;
                                        self.chat_error = None;
                                        self.status_message = format!(
                                            "Assistant settings saved to {}.",
                                            path.display()
                                        );
                                        // A fresh key may unlock the live model list.
                                        self.ensure_models_loaded();
                                    }
                                    Err(err) => self.chat_error = Some(err),
                                }
                            }
                            ui.weak("Stored locally, reused next launch.");
                        });
                        ui.separator();
                        ui.label("Model:");
                        ui.horizontal(|ui| {
                            if self.chat_models_loading {
                                ui.spinner();
                            }
                            let models = self.chat_models.clone();
                            if models.is_empty() && !self.chat_models_loading {
                                ui.weak("Paste an API key above to load models…");
                            } else {
                                egui::ComboBox::from_id_source("gemini_model")
                                    .selected_text(if self.chat_model.is_empty() {
                                        "Select…"
                                    } else {
                                        &self.chat_model
                                    })
                                    .show_ui(ui, |ui| {
                                        for m in &models {
                                            ui.selectable_value(
                                                &mut self.chat_model,
                                                m.clone(),
                                                m.as_str(),
                                            );
                                        }
                                    });
                            }
                            if ui
                                .button("⟳")
                                .on_hover_text("Reload model list from Google")
                                .clicked()
                            {
                                self.chat_models.clear();
                                self.chat_models_error = None;
                                self.ensure_models_loaded();
                            }
                        });
                        if let Some(err) = self.chat_models_error.clone() {
                            ui.weak(format!("{err} Using snapshot list."));
                        }
                        self.show_rate_limits(ui);
                        ui.add_space(4.0);
                        ui.label("Describe how to edit or write the Markdown:");
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

                        ui.horizontal(|ui| {
                            let send = egui::Button::new(
                                egui::RichText::new("✨ Send  (Ctrl+Enter)").strong(),
                            )
                            .fill(ACCENT);
                            ui.add_enabled(
                                !self.chat_busy && !self.chat_input.trim().is_empty(),
                                send,
                            )
                            .clicked()
                            .then(|| self.send_chat());
                            if self.chat_busy {
                                ui.spinner();
                                ui.label("Thinking…");
                            }
                        });

                        if let Some(err) = self.chat_error.clone() {
                            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
                        }

                        ui.separator();
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Suggestion:").strong());
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                if self.chat_response.is_empty() {
                                    ui.weak("No suggestion yet.");
                                } else {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.chat_response)
                                            .desired_rows(8)
                                            .desired_width(f32::INFINITY)
                                            .code_editor(),
                                    );
                                }
                            });

                        // Wrapped so the buttons flow instead of forcing
                        // the panel wider at small sizes.
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !self.chat_response.is_empty(),
                                    egui::Button::new("Insert at cursor"),
                                )
                                .clicked()
                            {
                                let snippet = self.chat_response.clone();
                                insert_at_cursor(self, ctx, &snippet);
                                self.status_message =
                                    String::from("Unsaved changes (Save file to enable autosave).");
                            }
                            if ui
                                .add_enabled(
                                    !self.chat_response.is_empty(),
                                    egui::Button::new("Replace document"),
                                )
                                .clicked()
                            {
                                self.content = self.chat_response.clone();
                                self.status_message =
                                    String::from("Unsaved changes (Save file to enable autosave).");
                            }
                            if ui.button("Clear").clicked() {
                                self.chat_response.clear();
                                self.chat_error = None;
                            }
                        });
                    });
            });
        if !self.tabs.assistant {
            // Dropped receiver (if any) just stops being polled; the
            // background thread's send will fail silently.
            self.chat_rx = None;
            self.chat_busy = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_instruction_and_document() {
        let p = build_prompt("Make it a table", "# Hi");
        assert!(p.contains("Make it a table"));
        assert!(p.contains("# Hi"));
        assert!(p.contains("Return ONLY"));
    }

    #[test]
    fn strips_markdown_fence_with_language_tag() {
        let out = strip_code_fence("```markdown\n# Hello\n```");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn strips_plain_fence_and_leaves_text_alone() {
        assert_eq!(strip_code_fence("```\nfoo\n```"), "foo");
        assert_eq!(strip_code_fence("# Hello"), "# Hello");
    }

    #[test]
    fn finds_limits_by_exact_and_prefixed_ids() {
        assert_eq!(find_limits("gemini-2.5-flash").unwrap().rpd, 20);
        assert_eq!(find_limits("models/gemini-3.1-flash-lite").unwrap().rpm, 15);
        assert_eq!(
            find_limits("gemini-2.5-flash-preview-05-20").unwrap().id,
            "gemini-2.5-flash"
        );
        assert!(find_limits("some-unknown-model").is_none());
    }

    #[test]
    fn formats_big_numbers_compactly() {
        assert_eq!(fmt_num(5), "5");
        assert_eq!(fmt_num(250_000), "250K");
    }

    #[test]
    fn fallback_list_only_contains_usable_models() {
        let models = fallback_models();
        assert!(models.contains(&"gemini-2.5-flash".to_string()));
        assert!(!models.contains(&"gemini-2.0-flash".to_string()));
    }
}
