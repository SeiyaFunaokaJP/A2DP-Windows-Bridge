"""Builds the BlueCaster icon from app/resources/icon-source.png and writes
app/resources/app.ico, the Tauri copy and the UI's app-icon.png.

    python app/resources/make_icon.py        (needs Pillow)

40 px and up are downscales of the artwork. Its fine arcs and wave smear below
that, so the tray sizes (16-32 px) are redrawn on their own: the same tile,
rune and arcs, with fewer and thicker lines.
"""
import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "app/resources/icon-source.png"
SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
SS = 8  # supersampling

# Taken from the artwork
TILE_LEFT = (0x02, 0x2E, 0x8C)
TILE_RIGHT = (0x3A, 0x12, 0xA6)
RIM_LEFT = (0x2C, 0x9B, 0xFF)
RIM_RIGHT = (0xA8, 0x5C, 0xFF)
RUNE = (0xD4, 0xF8, 0xFF, 255)
ARC_INNER = (0x4C, 0xE4, 0xFF, 255)
ARC_OUTER = (0xC4, 0x98, 0xFF, 255)


# 16 px: drawn pixel by pixel, a smooth shape is mush at that size
PIXELS_16 = """
................
....#...........
....##....bbb...
....###.....bb..
.##.#.##.....b..
..###.##..a...b.
...####....a..b.
....##.....a..b.
....##.....a..b.
...####....a..b.
..###.##..a...b.
.##.#.##.....b..
....###.....bb..
....##....bbb...
....#...........
................
"""
PIXEL_COLOURS = {"#": RUNE, "a": ARC_INNER, "b": ARC_OUTER}


def tile(size: int) -> Image.Image:
    n = size * SS
    rim = max(1.0, size * 0.04) * SS

    def gradient(left: tuple, right: tuple) -> Image.Image:
        img = Image.new("RGBA", (n, n))
        gd = ImageDraw.Draw(img)
        for x in range(n):
            t = x / (n - 1)
            gd.line([(x, 0), (x, n)], fill=tuple(round(a + (b - a) * t) for a, b in zip(left, right)) + (255,))
        return img

    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, n - 1, n - 1], radius=0.2 * n, fill=255)
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    img.paste(gradient(RIM_LEFT, RIM_RIGHT), (0, 0), mask)
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([rim, rim, n - 1 - rim, n - 1 - rim], radius=0.2 * n - rim, fill=255)
    img.paste(gradient(TILE_LEFT, TILE_RIGHT), (0, 0), mask)
    return img


def render16() -> Image.Image:
    img = tile(16).resize((16, 16), Image.LANCZOS)
    rows = PIXELS_16.strip("\n").splitlines()
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c in PIXEL_COLOURS:
                img.putpixel((x, y), PIXEL_COLOURS[c])
    return img


def render_small(size: int) -> Image.Image:
    if size == 16:
        return render16()
    n = size * SS
    u = lambda v: v * n  # noqa: E731  unit -> pixels
    img = tile(size)
    d = ImageDraw.Draw(img)
    stroke = round(max(1.5 / size, 0.07) * n)

    # Rune, left of centre
    rx, ry, a = 0.31, 0.5, 0.33
    b = a * 0.52
    pts = [(-b, -a / 2), (b, a / 2), (0, a), (0, -a), (b, -a / 2), (-b, a / 2)]
    d.line([(u(rx + x), u(ry + y)) for x, y in pts], fill=RUNE, width=stroke, joint="curve")

    # Arcs opening to the right; (Pillow draws the width inside the box)
    for r, colour in ((0.17, ARC_INNER), (0.34, ARC_OUTER)):
        ro = u(r) + stroke / 2
        cx, cy = u(0.52), u(0.5)
        d.arc([cx - ro, cy - ro, cx + ro, cy + ro], -80, 80, fill=colour, width=stroke)

    return img.resize((size, size), Image.LANCZOS)


def render(size: int, source: Image.Image) -> Image.Image:
    if size <= 32:
        return render_small(size)
    # The artwork is 1186 x 1171; 1 % of squash fills the square without a gap
    img = source.resize((size, size), Image.LANCZOS)
    return img if size >= 256 else img.filter(ImageFilter.UnsharpMask(radius=0.8, percent=60, threshold=0))


def load_source() -> Image.Image:
    img = Image.open(SOURCE).convert("RGBA")
    # The tile's alpha sits around 252: make it opaque, keep the soft edge
    img.putalpha(img.getchannel("A").point(lambda v: 255 if v >= 240 else v))
    return img


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
    source = load_source()
    images = {s: render(s, source) for s in SIZES}
    write_ico(ROOT / "app/resources/app.ico", images)
    write_ico(ROOT / "app/src/ui/src-tauri/icons/icon.ico", images)
    images[256].save(ROOT / "app/src/ui/public/app-icon.png", optimize=True)


if __name__ == "__main__":
    main()
