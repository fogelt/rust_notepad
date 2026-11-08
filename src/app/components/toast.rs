use crate::app::NotepadApp;
use crate::app::models::Toast;
use crate::app::theme::{ACCENT, BG_ELEVATED, TEXT};
use eframe::egui;
use egui_phosphor::regular as icons;
use std::time::{Duration, Instant};

/// How long a toast stays on screen before dismissing itself.
const TOAST_LIFETIME: Duration = Duration::from_secs(7);

impl NotepadApp {
    /// Show a transient toast above the footer. Replaces any current toast.
    pub fn push_toast(&mut self, message: impl Into<String>, is_error: bool) {
        self.toast = Some(Toast {
            message: message.into(),
            is_error,
            created: Instant::now(),
        });
    }

    pub fn show_toast(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        if toast.created.elapsed() >= TOAST_LIFETIME {
            self.toast = None;
            return;
        }

        let message = toast.message.clone();
        let is_error = toast.is_error;
        let edge = if is_error {
            ctx.style().visuals.error_fg_color
        } else {
            ACCENT
        };
        let icon = if is_error {
            icons::WARNING_CIRCLE
        } else {
            icons::CHECK_CIRCLE
        };

        let mut dismiss = false;
        egui::Area::new(egui::Id::new("app_toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -44.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(BG_ELEVATED)
                    .stroke(egui::Stroke::new(1.5_f32, edge))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 12.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(110),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(440.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon).size(18.0).color(edge));
                            ui.add(
                                egui::Label::new(egui::RichText::new(&message).color(TEXT))
                                    .wrap(true),
                            );
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(icons::X)
                                            .size(13.0)
                                            .color(ui.visuals().weak_text_color()),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Dismiss")
                                .clicked()
                            {
                                dismiss = true;
                            }
                        });
                    });
            });

        if dismiss {
            self.toast = None;
        }
        // Wake up in time to dismiss the toast even when nothing else repaints.
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}
