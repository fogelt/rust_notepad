# Patched `litehtml` 0.2.6

Upstream `litehtml` draws `<img>` elements at their intrinsic pixel size and
lets the host container clip them. With `width: 100%; height: auto`, that shows
a cropped top-left corner instead of a scaled image, and the crop changes as the
preview panel is resized.

`src/pixbuf.rs::draw_image` now:

- resamples the decoded image into the CSS border box (`resample_pixmap`, a
  bilinear scale into a scratch pixmap) and blits it 1:1. tiny-skia's
  `draw_pixmap` cannot scale and translate in one call: a scaled draw
  disappears once the element is further down the page.
- clips the image to the border box with its `border-radius`, so rounded
  photo boxes (background images) get rounded corners like solid fills do.

Images are laid out in CSS with a percentage `padding-top` box, so the box keeps
the photo's aspect ratio at every preview width.

`PixbufContainer` shares one cosmic-text `FontSystem` per thread. Creating one
scans and loads system fonts (tens of milliseconds), which dominated preview
renders when every container made its own.

Keep these patches when updating `litehtml`.
