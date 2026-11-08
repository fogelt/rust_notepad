"""Generate the Rusty Notepad app icon and .ico from one vector-ish drawing.

Run from the repo root:  py scripts/generate-icon.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 2048  # supersampled; downscaled for the final assets
ASSETS = Path(__file__).resolve().parent.parent / "assets"

ACCENT = "#4A78AD"
TILE = "#232A33"
TILE_BORDER = "#343D49"
PAGE = "#F6F1E7"
PAGE_QUOTE = "#ECE5D8"
INK = "#2A2C31"
WHITE = "#FFFFFF"


def rounded_box(
    draw: ImageDraw.ImageDraw,
    box: tuple[int, int, int, int],
    radius: int,
    fill: str,
    outline: str | None = None,
    width: int = 1,
) -> None:
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)

# App tile.
rounded_box(draw, (32, 32, SIZE - 32, SIZE - 32), 448, TILE, TILE_BORDER, 12)

# Paper sheet.
rounded_box(draw, (560, 330, 1480, 1710), 64, PAGE)

# Blue Markdown heading bar with a white # mark.
rounded_box(draw, (690, 470, 1350, 640), 40, ACCENT)
for x in (940, 1090):
    rounded_box(draw, (x - 16, 505, x + 16, 605), 16, WHITE)
for y in (535, 575):
    rounded_box(draw, (865, y - 16, 1165, y + 16), 16, WHITE)

# Body lines.
for box in (
    (690, 740, 1350, 800),
    (690, 860, 1350, 920),
    (690, 980, 1180, 1040),
):
    rounded_box(draw, box, 30, INK)

# Quote block.
rounded_box(draw, (690, 1120, 730, 1330), 20, ACCENT)
rounded_box(draw, (760, 1100, 1350, 1350), 40, PAGE_QUOTE)
for box in (
    (810, 1160, 1300, 1200),
    (810, 1250, 1160, 1290),
):
    rounded_box(draw, box, 20, INK)

# Task row.
rounded_box(draw, (690, 1450, 800, 1560), 24, ACCENT)
draw.line(
    [(720, 1505), (742, 1527), (775, 1480)],
    fill=WHITE,
    width=18,
    joint="curve",
)
rounded_box(draw, (830, 1485, 1350, 1525), 20, INK)

# Downscale for the window / package icon.
icon = image.resize((512, 512), Image.LANCZOS)
icon.save(ASSETS / "icon.png")

# Multi-resolution Windows icon.
image.resize((1024, 1024), Image.LANCZOS).save(
    ASSETS / "icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

print(f"Wrote {ASSETS / 'icon.png'} and {ASSETS / 'icon.ico'}")
