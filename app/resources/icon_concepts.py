"""BlueCaster icon concepts (issue #193): six candidate marks on the current
blue tile, each rendered at 256/48/32/16 px and laid out on one comparison
sheet, app/resources/icon-concepts.png.

    python app/resources/icon_concepts.py        (needs Pillow)

This is a concept round: nothing here touches app.ico, icon.ico or
app-icon.png. The 32 px and larger sizes are vector drawings (8x
supersampled); every 16 px version is a hand-placed pixel grid, as in
make_icon.py, because a rune or three arcs downscaled to 16 px smear.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
SS = 8  # supersampling

TOP = (0x35, 0x86, 0xEC)
BOTTOM = (0x1C, 0x58, 0xD0)
WHITE = (255, 255, 255, 255)
SIZES = [256, 48, 32, 16]


# ----------------------------------------------------------------- tile
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


def from_pixels(grid: str) -> Image.Image:
    """16 px tile with the grid's '#' cells painted white ('+' = 55 % white)."""
    img = tile(16, 0.0).resize((16, 16), Image.LANCZOS)
    rows = grid.strip("\n").splitlines()
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c == "#":
                img.putpixel((x, y), WHITE)
            elif c == "+":
                r, g, b, _ = img.getpixel((x, y))
                img.putpixel((x, y), tuple(round(v + (255 - v) * 0.55) for v in (r, g, b)) + (255,))
    return img


class Pen:
    """Unit-square drawing helpers on a supersampled tile."""

    def __init__(self, size: int):
        self.size = size
        self.n = size * SS
        self.img = tile(size, 0.04 if size <= 24 else 0.06)
        self.d = ImageDraw.Draw(self.img)
        # small sizes get relatively thicker lines so the tray icon stays crisp
        self.min_px = 1.6 if size <= 32 else 1.0

    def u(self, v: float) -> float:
        return v * self.n

    def w(self, units: float) -> int:
        return max(1, round(self.u(max(units, self.min_px / self.size))))

    def arc(self, cx, cy, r, a0, a1, width, fill=WHITE):
        wu = max(width, self.min_px / self.size)
        ro = r + wu / 2
        self.d.arc(
            [self.u(cx - ro), self.u(cy - ro), self.u(cx + ro), self.u(cy + ro)],
            a0, a1, fill=fill, width=self.w(width),
        )

    def line(self, pts, width, fill=WHITE):
        self.d.line([(self.u(x), self.u(y)) for x, y in pts], fill=fill, width=self.w(width), joint="curve")

    def dot(self, cx, cy, r, fill=WHITE):
        self.d.ellipse([self.u(cx - r), self.u(cy - r), self.u(cx + r), self.u(cy + r)], fill=fill)

    def rune(self, cx, cy, a, width, b=None):
        """Bluetooth rune centred on (cx, cy); a = half height."""
        b = a * 0.52 if b is None else b
        pts = [(-b, -a / 2), (b, a / 2), (0, a), (0, -a), (b, -a / 2), (-b, a / 2)]
        self.line([(cx + x, cy + y) for x, y in pts], width)

    def headphones(self, cx, cy, scale, band_w, rune=True, rune_w=None):
        """The current icon's headphones, (cx, cy) = band centre, scale 1 = make_icon.py."""
        r = 0.31 * scale
        cup_w = 0.15 * scale
        cup_top, cup_bot = cy + (0.52 - 0.56) * scale, cy + (0.84 - 0.56) * scale
        self.arc(cx, cy, r, 180, 360, band_w)
        for x in (cx - r, cx + r):
            self.line([(x, cy), (x, cy + 0.06 * scale)], band_w)
        for x in (cx - r + band_w / 2, cx + r - band_w / 2):
            x0 = x - cup_w / 2
            self.d.rounded_rectangle(
                [self.u(x0), self.u(cup_top), self.u(x0 + cup_w), self.u(cup_bot)],
                radius=self.u(cup_w * 0.42), fill=WHITE,
            )
        if rune:
            self.rune(cx, cy + 0.075 * scale, 0.20 * scale, rune_w or band_w * 0.75)

    def done(self) -> Image.Image:
        return self.img.resize((self.size, self.size), Image.LANCZOS)


# ------------------------------------------------------------- concepts
def concept_a(size: int) -> Image.Image:
    """A: bold rune, three transmit arcs leaving it to the right."""
    if size == 16:
        return from_pixels("""
................
................
....#...........
....##.....#....
....#.#...#.#...
..#.#.#..#..#.#.
...##....#..#.#.
....#....#..#.#.
...##....#..#.#.
..#.#.#..#..#.#.
....#.#...#.#...
....##.....#....
....#...........
................
................
................
""")
    p = Pen(size)
    big = size >= 48
    p.rune(0.34, 0.50, 0.30, 0.085 if big else 0.10, b=0.16)
    for r in (0.27, 0.38, 0.49) if big else (0.28, 0.42):
        p.arc(0.34, 0.50, r, -38, 38, 0.065 if big else 0.08)
    return p.done()


def concept_b(size: int) -> Image.Image:
    """B: the rune as a broadcast tower - stem becomes the mast, A-frame legs, two signal ticks."""
    if size == 16:
        return from_pixels("""
................
.......#........
.......##.......
...#...#.#...#..
..#..#.#.#..#...
......##........
.....#.#........
.....#.#.#......
....#.##.#......
.......#........
.......#........
......###.......
.....#.#.#......
....#..#..#.....
...#...#...#....
................
""")
    p = Pen(size)
    big = size >= 48
    w = 0.075 if big else 0.095
    cx, top, mid, base = 0.5, 0.13, 0.56, 0.90
    # rune, lobes only on the upper mast
    a = 0.21
    cy = top + a
    b = 0.14
    p.line([(cx - b, cy - a / 2), (cx + b, cy + a / 2), (cx, cy + a), (cx, cy - a), (cx + b, cy - a / 2), (cx - b, cy + a / 2)], w)
    # mast and A-frame legs
    p.line([(cx, cy + a), (cx, base)], w)
    p.line([(cx, mid + 0.02), (cx - 0.24, base)], w * 0.85)
    p.line([(cx, mid + 0.02), (cx + 0.24, base)], w * 0.85)
    p.line([(cx - 0.11, 0.76), (cx + 0.11, 0.76)], w * 0.7)
    # signal ticks either side of the top
    ty = top + 0.04
    radii = (0.20, 0.30) if big else (0.24,)
    for r in radii:
        p.arc(cx, ty, r, 160, 200, w * 0.7)
        p.arc(cx, ty, r, -20, 20, w * 0.7)
    return p.done()


def concept_c(size: int) -> Image.Image:
    """C: the current headphones, transmit arcs radiating above the band."""
    if size == 16:
        return from_pixels("""
................
.....######.....
...##......##...
..#..........#..
................
.....######.....
....#......#....
...#........#...
...#........#...
.###........###.
.###........###.
.###........###.
.###........###.
.###........###.
................
................
""")
    p = Pen(size)
    big = size >= 48
    band_w = 0.085 if big else 0.10
    scale = 0.86
    cx, cy = 0.5, 0.62
    p.headphones(cx, cy, scale, band_w, rune=big, rune_w=0.055)
    r = 0.31 * scale
    radii = (r + 0.12, r + 0.22) if big else (r + 0.15,)
    for rr in radii:
        p.arc(cx, cy, rr, 230, 310, band_w * 0.75)
    return p.done()


def concept_d(size: int) -> Image.Image:
    """D: a dot with waves fanning out both ways - signal, not hardware."""
    if size == 16:
        return from_pixels("""
................
................
................
.#...........#..
#.#.........#.#.
#..#...##..#..#.
#..#..####.#..#.
#..#..####.#..#.
#..#..####.#..#.
#..#...##..#..#.
#.#.........#.#.
.#...........#..
................
................
................
................
""")
    p = Pen(size)
    big = size >= 48
    cx, cy = 0.5, 0.5
    p.dot(cx, cy, 0.095)
    w = 0.07 if big else 0.09
    radii = (0.21, 0.31, 0.41) if big else (0.24, 0.38)
    for r in radii:
        p.arc(cx, cy, r, -40, 40, w)
        p.arc(cx, cy, r, 140, 220, w)
    return p.done()


def concept_e(size: int) -> Image.Image:
    """E: 'BC' monogram - the rune stands in for the B, a C hugs it."""
    if size == 16:
        return from_pixels("""
................
................
................
...#............
...##....####...
...#.#..#....#..
.#.#.#.#........
..##...#........
..##...#........
.#.#.#.#........
...#.#..#....#..
...##....####...
...#............
................
................
................
""")
    p = Pen(size)
    big = size >= 48
    w = 0.085 if big else 0.10
    p.rune(0.30, 0.50, 0.30, w, b=0.15)
    p.arc(0.66, 0.50, 0.21, 45, 315, w)
    return p.done()


def concept_f(size: int) -> Image.Image:
    """F: the rune casting a beam - a translucent cone to the right, one wave in it."""
    if size == 16:
        return from_pixels("""
................
................
................
....#......+++..
....##....+++++.
....#.#...+++++.
..#.#.#.#.+++++.
...##...#.+++++.
....#...#.+++++.
...##...#.+++++.
..#.#.#.#.+++++.
....#.#...+++++.
....##....+++++.
....#......+++..
................
................
""")
    p = Pen(size)
    big = size >= 48
    w = 0.085 if big else 0.10
    rx, ry = 0.33, 0.50
    # beam: a fan from the rune's right tips to the tile edge, two alpha steps
    beam = Image.new("RGBA", p.img.size, (0, 0, 0, 0))
    bd = ImageDraw.Draw(beam)
    u = p.u
    tip = (u(rx + 0.16), u(ry))
    bd.polygon([tip, (u(0.98), u(0.06)), (u(0.98), u(0.94))], fill=(255, 255, 255, 80))
    bd.polygon([tip, (u(0.98), u(0.24)), (u(0.98), u(0.76))], fill=(255, 255, 255, 80))
    # clip the beam to the tile's rounded corners
    mask = p.img.split()[3]
    beam.putalpha(Image.composite(beam.split()[3], Image.new("L", beam.size, 0), mask))
    p.img.alpha_composite(beam)
    p.d = ImageDraw.Draw(p.img)
    p.rune(rx, ry, 0.30, w, b=0.16)
    p.arc(rx, ry, 0.40 if big else 0.42, -30, 30, w * 0.8)
    return p.done()


CONCEPTS = [
    ("A", "ルーン + 送信アーク", concept_a),
    ("B", "ルーンの電波塔", concept_b),
    ("C", "ヘッドホン + 送信アーク", concept_c),
    ("D", "ドットから広がる波", concept_d),
    ("E", "BC モノグラム", concept_e),
    ("F", "ルーンのビーム", concept_f),
]


# ---------------------------------------------------------------- sheet
def font(size: int) -> ImageFont.FreeTypeFont:
    for name in ("meiryob.ttc", "meiryo.ttc", "YuGothB.ttc", "msgothic.ttc", "segoeui.ttf"):
        path = Path("C:/Windows/Fonts") / name
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.load_default()


def sheet() -> Image.Image:
    f_big, f_small = font(22), font(14)
    pad, row_h = 28, 300
    col_x = {"label": 24, "256": 24, "swatch": 320}
    swatch_w, swatch_h = 250, 120
    width = 24 + 256 + 40 + swatch_w + 24
    height = 70 + row_h * len(CONCEPTS) + pad
    img = Image.new("RGBA", (width, height), (236, 239, 245, 255))
    d = ImageDraw.Draw(img)
    d.text((24, 20), "BlueCaster アイコン案   256 / 48 / 32 / 16 px (16 は 4 倍拡大も)", fill=(30, 30, 40), font=f_big)

    for i, (key, title, fn) in enumerate(CONCEPTS):
        y0 = 70 + i * row_h
        d.text((col_x["label"], y0), f"{key}  {title}", fill=(30, 30, 40), font=f_big)
        imgs = {s: fn(s) for s in SIZES}
        img.alpha_composite(imgs[256], (col_x["256"], y0 + 34))
        # one light and one dark swatch with the small sizes, as the taskbar / tray show them
        for j, (bg, fg) in enumerate((((255, 255, 255, 255), (60, 60, 70)), ((32, 32, 36, 255), (200, 200, 210)))):
            sx = col_x["swatch"]
            sy = y0 + 34 + j * (swatch_h + 16)
            d.rounded_rectangle([sx, sy, sx + swatch_w, sy + swatch_h], radius=10, fill=bg)
            x = sx + 16
            for s in (48, 32, 16):
                im = imgs[s]
                img.alpha_composite(im, (x, sy + (swatch_h - 20) // 2 - s // 2 + 6))
                d.text((x, sy + swatch_h - 22), f"{s}", fill=fg, font=f_small)
                x += s + 22
            big16 = imgs[16].resize((64, 64), Image.NEAREST)
            img.alpha_composite(big16, (x + 8, sy + (swatch_h - 20) // 2 - 32 + 6))
            d.text((x + 8, sy + swatch_h - 22), "16 ×4", fill=fg, font=f_small)
    return img


def main() -> None:
    out = HERE / "icon-concepts.png"
    sheet().save(out, optimize=True)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
