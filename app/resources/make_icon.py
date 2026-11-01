"""Builds the BlueCaster icon from app/resources/icon-source.png and writes
app/resources/app.ico, the Tauri copy and the UI's app-icon.png.

    python app/resources/make_icon.py        (needs Pillow)

Every size is a downscale of the artwork.
"""
import io
import struct
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "app/resources/icon-source.png"
SIZES = [16, 20, 24, 32, 40, 48, 64, 256]


def render(size: int, source: Image.Image) -> Image.Image:
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
