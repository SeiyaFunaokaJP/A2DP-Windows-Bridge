"""Builds the BlueCaster icon from app/resources/icon-source.png and writes
app/resources/app.ico, the Tauri copy and the UI's app-icon.png.

    python app/resources/make_icon.py        (needs Pillow)

Every size is a downscale of the artwork. At the tray sizes (16-32 px) its
lines are thickened first and the contrast lifted, so the rune and arcs
stay readable instead of a blue square (issue #86); still the artwork, not
a redraw, which looked out of place next to it.
"""
import io
import struct
from pathlib import Path

from PIL import Image, ImageEnhance, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "app/resources/icon-source.png"
SIZES = [16, 20, 24, 32, 40, 48, 64, 256]


# How far the bright lines grow (a max filter on the artwork, odd) before
# shrinking to each tray size: about one output pixel's worth
THICKEN = {16: 13, 20: 11, 24: 9, 32: 7}


def render_small(size: int, source: Image.Image) -> Image.Image:
    rgb = source.convert("RGB").filter(ImageFilter.MaxFilter(THICKEN[size]))
    img = Image.merge("RGBA", (*rgb.split(), source.getchannel("A"))).resize((size, size), Image.LANCZOS)
    rgb = ImageEnhance.Brightness(ImageEnhance.Contrast(img.convert("RGB")).enhance(1.35)).enhance(1.1)
    img = Image.merge("RGBA", (*rgb.split(), img.getchannel("A")))
    return img.filter(ImageFilter.UnsharpMask(radius=0.6, percent=80, threshold=0))


def render(size: int, source: Image.Image) -> Image.Image:
    if size in THICKEN:
        return render_small(size, source)
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
