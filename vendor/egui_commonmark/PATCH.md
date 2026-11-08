# Local patch

Upstream crate: [`egui_commonmark` 0.14.0](https://crates.io/crates/egui_commonmark)
(MIT OR Apache-2.0). Only rendering details are changed:

1. **Task lists** — upstream draws the list bullet when an item starts, before
   it sees the `TaskListMarker` event, so task items showed a bullet *and* a
   checkbox. The bullet rect is now recorded in `List::start_item`;
   `List::take_last_bullet` erases it and the checkbox is drawn in its spot.
2. **Line height** — preview text is laid out with a little extra leading
   (`apply_line_height`) so paragraphs are easier to read.
3. **Code copy button** — inset fully inside the code block (including one-line
   blocks), with a pointing-hand cursor and a short "Copied" toast.

Everything else is unmodified.
