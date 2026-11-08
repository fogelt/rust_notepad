# Local patch

Upstream crate: [`egui_commonmark` 0.14.0](https://crates.io/crates/egui_commonmark)
(MIT OR Apache-2.0). Only rendering details are changed:

1. **Task lists** — upstream draws the list bullet when an item starts, before
   it sees the `TaskListMarker` event, so task items showed a bullet *and* a
   checkbox. The bullet rect is now recorded in `List::start_item`;
   `List::take_last_bullet` erases it and the checkbox is drawn in its spot.
2. **Line height** – preview text is laid out with configurable leading
   (`CommonMarkViewer::line_spacing`), applied to body text and fenced code
   blocks so they follow the same setting.
3. **Code copy button** – inset fully inside the code block (including one-line
   blocks), with a pointing-hand cursor and a short "Copied" toast.
4. **Code block spacing** – a little extra margin after fenced code blocks so
   following paragraphs do not sit tight against them.

Everything else is unmodified.
