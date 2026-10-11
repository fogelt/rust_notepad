use eframe::egui;

pub mod components;
pub mod config;
pub mod models;
pub mod theme;

pub use models::NotepadApp;

use theme::WINDOW_MIN_SIZE;

impl NotepadApp {
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
}

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.snap_to_min_on_monitor_change(ctx);
        self.enforce_min_window_size(ctx);
        self.show_top_bar(ctx);
        self.show_markdown_toolbar(ctx);
        self.show_chat_panel(ctx);
        // Declared after the side panels so the footer paints last
        // (on top of everything) and spans the width between them.
        self.show_bottom_bar(ctx);
        self.show_editor(ctx);
    }
}
