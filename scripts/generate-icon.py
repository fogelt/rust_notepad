"""Generate the Rusty Notepad app icon and .ico from one vector-ish drawing.

Run from the repo root:  py scripts/generate-icon.py
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

SIZE = 2048  # supersampled; downscaled for the final assets
ASSETS = Path(__file__).resolve().parent.parent / "assets"

ACCENT = "#4A78AD"
PAGE = "#F6F1E7"
PAGE_BORDER = "#2C3A4A"
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


def bold_font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for path in (
        "C:/Windows/Fonts/segoeuib.ttf",
        "C:/Windows/Fonts/arialbd.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    ):
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            pass
    return ImageFont.load_default()


image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)

# Large cream page with a transparent surround and a blue bookmark ribbon.
rounded_box(draw, (180, 120, 1868, 1900), 120, PAGE, PAGE_BORDER, 14)
draw.polygon(
    [(1460, 120), (1710, 120), (1710, 620), (1585, 515), (1460, 620)],
    fill=ACCENT,
)

# The RMD brand mark.
draw.text(
    (980, 900),
    "RMD",
    font=bold_font(420),
    fill=ACCENT,
    anchor="mm",
)

# Document lines below the mark.
for box in (
    (500, 1240, 1600, 1300),
    (500, 1390, 1460, 1450),
    (500, 1540, 1300, 1600),
):
    rounded_box(draw, box, 30, INK)

# Downscale for the window / package icon.
icon = image.resize((512, 512), Image.LANCZOS)
icon.save(ASSETS / "icon.png")

# Multi-resolution Windows icon.
image.resize((1024, 1024), Image.LANCZOS).save(
    ASSETS / "icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

print(f"Wrote {ASSETS / 'icon.png'} and {ASSETS / 'icon.ico'}")
