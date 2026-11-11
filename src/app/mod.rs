use eframe::egui;

pub mod audio;
pub mod components;
pub mod config;
pub mod keys;
pub mod live;
pub mod local_stt;
pub mod models;
pub mod provider_stt;
pub mod templates;
pub mod theme;

pub use models::NotepadApp;

use theme::WINDOW_MIN_SIZE;

impl NotepadApp {
    /// Grow the startup window to a comfortable fraction of the monitor,
    /// keeping it centered where the OS opened it. Runs once.
    fn apply_initial_window_size(&mut self, ctx: &egui::Context) {
        if self.initial_window_sized {
            return;
        }
        let (monitor, inner, outer) = ctx.input(|i| {
            let viewport = i.viewport();
            (
                viewport.monitor_size,
                viewport.inner_rect,
                viewport.outer_rect,
            )
        });
        let (Some(monitor), Some(inner), Some(outer)) = (monitor, inner, outer) else {
            return;
        };
        if monitor.x <= 0.0 || monitor.y <= 0.0 {
            return;
        }
        self.initial_window_sized = true;

        let minimum = egui::vec2(WINDOW_MIN_SIZE[0], WINDOW_MIN_SIZE[1]);
        let target = egui::vec2(
            (monitor.x * 0.8).clamp(900.0, 1480.0),
            (monitor.y * 0.8).clamp(600.0, 980.0),
        )
        .max(minimum);

        // Keep the centre the OS chose for the initial min-size window.
        let chrome = outer.size() - inner.size();
        let chrome = egui::vec2(chrome.x.max(0.0), chrome.y.max(0.0));
        let target_outer = target + chrome;
        let position = outer.center() - target_outer * 0.5;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
    }

    /// Shrink the window back to the minimum size when it moves to a
    /// different monitor (or the DPI changes), instead of letting the OS
    /// resize jump it around. Fires once per change, never every frame.
    /// Skipped while maximized/minimized/fullscreen so we never disturb
    /// those states.
    fn snap_to_min_on_monitor_change(&mut self, ctx: &egui::Context) {
        let (monitor, ppp, maximized, minimized, fullscreen) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.monitor_size.map(|m| [m.x, m.y]),
                v.native_pixels_per_point,
                v.maximized.unwrap_or(false),
                v.minimized.unwrap_or(false),
                v.fullscreen.unwrap_or(false),
            )
        });
        let changed = match (self.last_monitor_size, monitor) {
            (Some(prev), Some(cur)) => prev != cur,
            _ => false,
        } || match (self.last_native_ppp, ppp) {
            (Some(prev), Some(cur)) => prev != cur,
            _ => false,
        };
        if monitor.is_some() {
            self.last_monitor_size = monitor;
        }
        if ppp.is_some() {
            self.last_native_ppp = ppp;
        }
        if changed && !(maximized || minimized || fullscreen) {
            eprintln!(
                "[monitor] screen changed (size={monitor:?}, ppp={ppp:?}); snapping to min size"
            );
            let min = egui::vec2(WINDOW_MIN_SIZE[0], WINDOW_MIN_SIZE[1]);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(min));
            // Re-register the floor in the new screen's DPI context so the
            // next manual resize can't snap up to a stale limit.
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
        }
    }

    /// Backstop for the OS minimum: if the window somehow ends up smaller
    /// than the floor (e.g. the minimum isn't enforced mid drag-resize),
    /// side panels get squeezed below their minimums and headers clip.
    /// Snap the deficient axes back; a no-op otherwise.
    fn enforce_min_window_size(&mut self, ctx: &egui::Context) {
        let (inner, maximized, minimized, fullscreen) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect.map(|r| [r.width(), r.height()]),
                v.maximized.unwrap_or(false),
                v.minimized.unwrap_or(false),
                v.fullscreen.unwrap_or(false),
            )
        });
        if maximized || minimized || fullscreen {
            return;
        }
        if let Some([w, h]) = inner
            && (w < WINDOW_MIN_SIZE[0] - 1.0 || h < WINDOW_MIN_SIZE[1] - 1.0)
        {
            eprintln!("[window] below minimum ({w:.0}x{h:.0}); restoring floor");
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                WINDOW_MIN_SIZE[0].max(w),
                WINDOW_MIN_SIZE[1].max(h),
            )));
        }
    }

    /// Keep the OS window title in sync with the open document. Only sends a
    /// viewport command when the title actually changes.
    fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = match self.file_path.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("{} — Rusty Notepad", name.to_string_lossy()),
            None => String::from("Rusty Notepad"),
        };
        if self.window_title.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = Some(title);
        }
    }

    /// F11 toggles real fullscreen (Esc leaves it). Handy for reading the
    /// preview with zero chrome.
    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        let toggle = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11));
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        // Only claim Esc while fullscreen; otherwise it belongs to menus/popups.
        let escape = fullscreen
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if toggle || escape {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
    }
}

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_initial_window_size(ctx);
        self.snap_to_min_on_monitor_change(ctx);
        self.enforce_min_window_size(ctx);
        self.update_window_title(ctx);
        self.handle_global_shortcuts(ctx);
        // Audio files can be dropped anywhere; transcribe them locally.
        self.handle_dropped_audio(ctx);
        self.poll_dictation(ctx);
        self.show_top_bar(ctx);
        // The markdown toolbar only acts on the editor, so don't waste vertical
        // space on it while the editor is hidden (e.g. full-width preview).
        if self.tabs.editor {
            self.show_markdown_toolbar(ctx);
        }
        // The Assistant is a full-height side panel. The footer is declared
        // after it so it only spans the editor/preview area.
        self.show_chat_panel(ctx);
        self.show_bottom_bar(ctx);
        self.show_editor(ctx);
        // Toasts live on the foreground layer, above every panel.
        self.show_toast(ctx);
        self.show_audio_drop_overlay(ctx);
    }
}
