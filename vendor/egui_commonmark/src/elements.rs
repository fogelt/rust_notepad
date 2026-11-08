use egui::{self, epaint, NumExt, RichText, Sense, TextStyle, Ui, Vec2};

pub(crate) fn soft_break(ui: &mut Ui) {
    ui.label(" ");
}

pub(crate) fn newline(ui: &mut Ui) {
    ui.label("\n");
}

pub(crate) fn bullet_point(ui: &mut Ui) -> egui::Rect {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().circle_filled(
        rect.center(),
        rect.height() / 6.0,
        ui.visuals().strong_text_color(),
    );
    rect
}

pub(crate) fn bullet_point_hollow(ui: &mut Ui) -> egui::Rect {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().circle(
        rect.center(),
        rect.height() / 6.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(0.6_f32, ui.visuals().strong_text_color()),
    );
    rect
}

pub(crate) fn number_point(ui: &mut Ui, number: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        format!("{number}. "),
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

pub(crate) fn footnote_start(ui: &mut Ui, note: &str) {
    ui.label(RichText::new(note).raised().strong().small());
}

pub(crate) fn footnote(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width_body_space(ui) * 4.0, height_body(ui)),
        Sense::hover(),
    );
    ui.painter().text(
        rect.right_top(),
        egui::Align2::RIGHT_TOP,
        format!("{text}."),
        TextStyle::Small.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
}

fn height_body(ui: &Ui) -> f32 {
    ui.text_style_height(&TextStyle::Body)
}

fn width_body_space(ui: &Ui) -> f32 {
    let id = TextStyle::Body.resolve(ui.style());
    ui.fonts(|f| f.glyph_width(&id, ' '))
}

/// Enhanced/specialized version of egui's code blocks. This one features copy button and borders
pub fn code_block<'t>(
    ui: &mut Ui,
    max_width: f32,
    text: &str,
    layouter: &'t mut dyn FnMut(&Ui, &str, f32) -> std::sync::Arc<egui::Galley>,
) {
    let mut text = text.strip_suffix('\n').unwrap_or(text);

    // To manually add background color to the code block, we imitate what
    // TextEdit does internally
    let where_to_put_background = ui.painter().add(egui::Shape::Noop);

    // Reserve room on the right for the copy button so it never overlaps code
    // (or pokes outside a one-line block).
    let button_space = 58.0;
    let inner_width = (max_width - button_space).max(80.0);

    // We use a `TextEdit` to make the text selectable.
    // Note that we take a `&mut` to a non-`mut` `&str`, which is
    // the how to tell `egui` that the text is not editable.
    let output = egui::TextEdit::multiline(&mut text)
        .layouter(layouter)
        .desired_width(inner_width)
        .margin(egui::Margin {
            left: 12.0,
            right: 46.0,
            top: 8.0,
            bottom: 8.0,
        })
        // prevent trailing lines
        .desired_rows(1)
        .show(ui);

    // Background color + frame (This is lost when TextEdit it not editable)
    let frame_rect = output.response.rect;
    ui.painter().set(
        where_to_put_background,
        epaint::RectShape::new(
            frame_rect,
            ui.style().noninteractive().rounding,
            ui.visuals().extreme_bg_color,
            ui.visuals().widgets.noninteractive.bg_stroke,
        ),
    );

    // Copy button, inset from the top-right corner and fully inside the frame.
    // One-line blocks center it vertically; longer blocks keep it at the top.
    let button_size = egui::vec2(30.0, 22.0);
    let button_center_y = if text.contains('\n') {
        frame_rect.top() + 8.0 + button_size.y * 0.5
    } else {
        frame_rect.center().y
    };
    let copy_button_rect = egui::Rect::from_center_size(
        egui::pos2(frame_rect.right() - 25.0, button_center_y),
        button_size,
    );
    let toast_id = egui::Id::new(("egui_commonmark_copied", text));
    let now = ui.ctx().input(|i| i.time);
    let copy_button = ui
        .put(
            copy_button_rect,
            egui::Button::new("🗐")
                .small()
                .frame(false)
                .fill(egui::Color32::TRANSPARENT),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Copy code");

    if copy_button.clicked() {
        use egui::TextBuffer as _;
        let copy_text = if let Some(cursor) = output.cursor_range {
            let selected_chars = cursor.as_sorted_char_range();
            let selected_text = text.char_range(selected_chars);
            if selected_text.is_empty() {
                text.to_owned()
            } else {
                selected_text.to_owned()
            }
        } else {
            text.to_owned()
        };
        ui.ctx().copy_text(copy_text);
        ui.ctx().data_mut(|d| d.insert_temp(toast_id, now));
        ui.ctx().request_repaint();
    }

    // Small "Copied" toast next to the button for a moment.
    if let Some(copied_at) = ui.ctx().data(|d| d.get_temp::<f64>(toast_id)) {
        if now - copied_at < 1.6 {
            let galley = ui.painter().layout_no_wrap(
                "Copied".to_owned(),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
            let padding = egui::vec2(8.0, 4.0);
            let size = galley.size() + 2.0 * padding;
            let left =
                (copy_button.rect.left() - size.x - 6.0).max(frame_rect.left() + 4.0);
            let toast_rect = egui::Rect::from_min_size(
                egui::pos2(left, copy_button.rect.center().y - size.y * 0.5),
                size,
            );
            ui.painter().rect(
                toast_rect,
                egui::Rounding::same(6.0),
                ui.visuals().selection.bg_fill,
                egui::Stroke::NONE,
            );
            ui.painter()
                .galley(toast_rect.min + padding, galley, egui::Color32::WHITE);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

// Stripped down version of egui's Checkbox. The only difference is that this
// creates a noninteractive checkbox. ui.add_enabled could have been used instead,
// but it makes the checkbox too grey.
pub(crate) struct Checkbox<'a> {
    checked: &'a mut bool,
}

impl<'a> Checkbox<'a> {
    pub fn without_text(checked: &'a mut bool) -> Self {
        Checkbox { checked }
    }
}

impl<'a> egui::Widget for Checkbox<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let Checkbox { checked } = self;

        let spacing = &ui.spacing();
        let icon_width = spacing.icon_width;

        let mut desired_size = egui::vec2(icon_width, 0.0);
        desired_size = desired_size.at_least(Vec2::splat(spacing.interact_size.y));
        desired_size.y = desired_size.y.max(icon_width);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().visuals.noninteractive();
            let (small_icon_rect, big_icon_rect) = ui.spacing().icon_rectangles(rect);
            ui.painter().add(epaint::RectShape::new(
                big_icon_rect.expand(visuals.expansion),
                visuals.rounding,
                visuals.bg_fill,
                visuals.bg_stroke,
            ));

            if *checked {
                // Check mark:
                ui.painter().add(egui::Shape::line(
                    vec![
                        egui::pos2(small_icon_rect.left(), small_icon_rect.center().y),
                        egui::pos2(small_icon_rect.center().x, small_icon_rect.bottom()),
                        egui::pos2(small_icon_rect.right(), small_icon_rect.top()),
                    ],
                    visuals.fg_stroke,
                ));
            }
        }

        response
    }
}

pub(crate) struct ListLevel {
    current_number: Option<u64>,
}

#[derive(Default)]
pub(crate) struct List {
    items: Vec<ListLevel>,
    /// Rect of the most recently drawn bullet, so a task marker can replace it.
    last_bullet: Option<egui::Rect>,
}

impl List {
    pub fn start_level_with_number(&mut self, start_number: u64) {
        self.items.push(ListLevel {
            current_number: Some(start_number),
        });
    }

    pub fn start_level_without_number(&mut self) {
        self.items.push(ListLevel {
            current_number: None,
        });
    }

    pub fn is_inside_a_list(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn start_item(&mut self, ui: &mut Ui, options: &crate::CommonMarkOptions) {
        if self.items.is_empty() {
            unreachable!();
        }
        let len = self.items.len();
        newline(ui);
        ui.label(" ".repeat((len - 1) * options.indentation_spaces));

        let number = self.items.last_mut().and_then(|item| {
            let number = item.current_number?;
            item.current_number = Some(number + 1);
            Some(number)
        });
        self.last_bullet = if let Some(number) = number {
            number_point(ui, &number.to_string());
            None
        } else if len > 1 {
            Some(bullet_point_hollow(ui))
        } else {
            Some(bullet_point(ui))
        };
    }

    /// Take the bullet drawn for the current item and erase it, returning its
    /// rect. Task-list checkboxes are drawn in this spot instead of a bullet.
    pub fn take_last_bullet(&mut self, ui: &mut Ui) -> Option<egui::Rect> {
        let rect = self.last_bullet.take()?;
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().panel_fill);
        Some(rect)
    }

    pub fn end_level(&mut self, ui: &mut Ui) {
        self.items.pop();

        if self.items.is_empty() {
            newline(ui);
        }
    }
}

pub(crate) fn blockquote(ui: &mut Ui, accent: egui::Color32, add_contents: impl FnOnce(&mut Ui)) {
    newline(ui);
    let start = ui.painter().add(egui::Shape::Noop);
    let response = egui::Frame::none()
        // offset the frame so that we can use the space for the horizontal line and other stuff
        // By not using a separator we have better control
        .outer_margin(egui::Margin {
            left: 10.0,
            ..Default::default()
        })
        .show(ui, add_contents)
        .response;

    // FIXME: Add some rounding

    ui.painter().set(
        start,
        egui::epaint::Shape::LineSegment {
            points: [
                egui::pos2(response.rect.left_top().x, response.rect.left_top().y + 5.0),
                egui::pos2(
                    response.rect.left_bottom().x,
                    response.rect.left_bottom().y - 5.0,
                ),
            ],
            stroke: egui::Stroke::new(3.0_f32, accent),
        },
    );
}
