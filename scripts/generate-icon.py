"""Generate the Rusty Notepad app icon and .ico from one vector-ish drawing.

Run from the repo root:  py scripts/generate-icon.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 2048  # supersampled; downscaled for the final assets
ASSETS = Path(__file__).resolve().parent.parent / "assets"

ACCENT = "#4A78AD"
TILE = "#1C222B"
TILE_BORDER = "#2C3A4A"
PAGE = "#F6F1E7"
INK = "#2A2C31"


def rounded_box(
    draw: ImageDraw.ImageDraw,
    box: tuple[int, int, int, int],
    radius: int,
    fill: str,
    outline: str | None = None,
    width: int = 1,
) -> None:
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def hash_mark(
    draw: ImageDraw.ImageDraw,
    center: tuple[int, int],
    width: int,
    height: int,
    thickness: int,
    color: str,
) -> None:
    cx, cy = center
    for x in (cx - width // 3, cx + width // 3):
        rounded_box(
            draw,
            (x - thickness // 2, cy - height // 2, x + thickness // 2, cy + height // 2),
            thickness // 2,
            color,
        )
    for y in (cy - height // 3, cy + height // 3):
        rounded_box(
            draw,
            (cx - width // 2, y - thickness // 2, cx + width // 2, y + thickness // 2),
            thickness // 2,
            color,
        )


image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)

# App tile.
rounded_box(draw, (32, 32, SIZE - 32, SIZE - 32), 448, TILE, TILE_BORDER, 12)

# Cream page with a blue bookmark ribbon.
rounded_box(draw, (500, 360, 1548, 1688), 96, PAGE)
draw.polygon(
    [(1200, 360), (1440, 360), (1440, 800), (1320, 700), (1200, 800)],
    fill=ACCENT,
)

# Bold Markdown # mark.
hash_mark(draw, (940, 1060), 500, 500, 76, ACCENT)

# A single page line anchors the mark.
rounded_box(draw, (660, 1420, 1388, 1480), 30, INK)

# Downscale for the window / package icon.
icon = image.resize((512, 512), Image.LANCZOS)
icon.save(ASSETS / "icon.png")

# Multi-resolution Windows icon.
image.resize((1024, 1024), Image.LANCZOS).save(
    ASSETS / "icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

print(f"Wrote {ASSETS / 'icon.png'} and {ASSETS / 'icon.ico'}")
