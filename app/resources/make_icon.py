"""Draws the BlueCaster icon (headphones with the Bluetooth rune on a blue tile)
and writes app/resources/app.ico, the Tauri copy and the UI's app-icon.png.

    python app/resources/make_icon.py        (needs Pillow)

Each size is drawn on its own (8x supersampled) so that the 16/24 px tray
sizes get strokes of at least ~1.5 px instead of a blurred downscale.
"""
import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
SS = 8  # supersampling

TOP = (0x35, 0x86, 0xEC)
BOTTOM = (0x1C, 0x58, 0xD0)
WHITE = (255, 255, 255, 255)


# 16 px: drawn pixel by pixel, a smooth shape is mush at that size
PIXELS_16 = """
................
................
.....######.....
...##......##...
..#..........#..
..#....#.....#..
..#....##....#..
..#..#.#.#...#..
.###..###...###.
.###...#....###.
.###..###...###.
.###.#.#.#..###.
.###...##...###.
.###...#....###.
................
................
"""


def tile(size: int, inset: float) -> Image.Image:
    n = size * SS
    grad = Image.new("RGBA", (n, n))
    gd = ImageDraw.Draw(grad)
    for y in range(n):
        t = y / (n - 1)
        gd.line([(0, y), (n, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(TOP, BOTTOM)) + (255,))
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [inset * n, inset * n, (1 - inset) * n, (1 - inset) * n], radius=0.2 * n, fill=255
    )
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    img.paste(grad, (0, 0), mask)
    return img


def render16() -> Image.Image:
    img = tile(16, 0.0).resize((16, 16), Image.LANCZOS)
    rows = PIXELS_16.strip("\n").splitlines()
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c == "#":
                img.putpixel((x, y), WHITE)
    return img


def render(size: int) -> Image.Image:
    if size == 16:
        return render16()
    n = size * SS
    u = lambda v: v * n  # noqa: E731  unit -> pixels

    # Strokes in units; small sizes get relatively thicker lines
    min_px = 1.6 if size <= 24 else 1.0
    band_w = max(0.075, min_px / size)
    rune_w = max(0.055, min_px * 0.85 / size)

    img = tile(size, 0.04 if size <= 24 else 0.06)
    d = ImageDraw.Draw(img)

    # Headband: upper half of a circle. dy centres the drawing (band top to
    # the lower of cups and rune) vertically on the tile
    cx, r = 0.5, 0.31
    cup_w, cup_top, cup_bot = 0.15, 0.52, 0.84
    a = 0.20 if size > 24 else 0.22  # rune half height
    bottom = max(cup_bot, 0.635 + a + rune_w / 2)
    dy = 0.5 - (0.56 - r - band_w / 2 + bottom) / 2
    cy, cup_top, cup_bot = 0.56 + dy, cup_top + dy, cup_bot + dy
    # (Pillow draws the width inside the box: grow it so the stroke is centred on r)
    ro = r + band_w / 2
    d.arc([u(cx - ro), u(cy - ro), u(cx + ro), u(cy + ro)], 180, 360, fill=WHITE, width=round(u(band_w)))
    # Band ends run straight down into the cups
    for x in (cx - r, cx + r):
        d.line([(u(x), u(cy)), (u(x), u(0.62 + dy))], fill=WHITE, width=round(u(band_w)))

    # Ear cups
    for x in (cx - r + band_w / 2, cx + r - band_w / 2):
        x0 = x - cup_w / 2
        d.rounded_rectangle(
            [u(x0), u(cup_top), u(x0 + cup_w), u(cup_bot)], radius=u(cup_w * 0.42), fill=WHITE
        )

    # Bluetooth rune between the cups
    rx, ry = 0.5, 0.635 + dy
    b = a * 0.52  # half width
    pts = [(-b, -a / 2), (b, a / 2), (0, a), (0, -a), (b, -a / 2), (-b, a / 2)]
    d.line([(u(rx + x), u(ry + y)) for x, y in pts], fill=WHITE, width=round(u(rune_w)), joint="curve")

    return img.resize((size, size), Image.LANCZOS)


def png_bytes(im: Image.Image) -> bytes:
    buf = io.BytesIO()
    im.save(buf, "PNG", optimize=True)
    return buf.getvalue()


def write_ico(path: Path, images: dict[int, Image.Image]) -> None:
    blobs = [(s, png_bytes(images[s])) for s in SIZES]
    out = bytearray(struct.pack("<HHH", 0, 1, len(blobs)))
    off = 6 + 16 * len(blobs)
    for s, blob in blobs:
        dim = 0 if s >= 256 else s
        out += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), off)
        off += len(blob)
    for _, blob in blobs:
        out += blob
    path.write_bytes(bytes(out))


def main() -> None:
    images = {s: render(s) for s in SIZES}
    write_ico(ROOT / "app/resources/app.ico", images)
    write_ico(ROOT / "app/src/ui/src-tauri/icons/icon.ico", images)
    images[256].save(ROOT / "app/src/ui/public/app-icon.png", optimize=True)


if __name__ == "__main__":
    main()
