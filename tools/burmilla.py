#!/usr/bin/env python3
"""Generate the Burmilla sprite sheet from the classic 1-bit neko sprites.

The classic sprites (sprites/classic/*.png) are 32x32, black outline on white.
This tool:

  1. classifies every pixel (transparent, outline, inner line, fur, eye, nose),
  2. upscales the class map 4x with Scale2x twice so diagonals stay smooth,
     then thins the lines back to 2px so the cat stays crisp,
  3. paints a Burmilla coat at 128x128 (1:1 on a 2x HiDPI screen at the
     default size): silver-white undercoat with dark tipping
     along the back, head and tail, a pale chest, green eyes rimmed in black
     and a brick-pink nose,
  4. derives extra frames (blinks) and writes assets/burmilla.png.

It also writes assets/classic.png: the original black-and-white look at the
same 128x128 size and layout, for anyone who wants the 1989 cat back.

The frame order is the FRAMES list below; src/sprites.rs mirrors it.

Usage: python3 tools/burmilla.py [--preview out.png]
"""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "sprites" / "classic"
ASSETS = ROOT / "assets"

SRC_SIZE = 32
SCALE = 4
SIZE = SRC_SIZE * SCALE
COLUMNS = 8

# (frame name, source sprite, variant)
FRAMES = [
    ("awake", "awake", None),
    ("awake_blink", "awake", "blink"),
    ("scratch1", "scratch1", None),
    ("scratch2", "scratch2", None),
    ("wash", "wash1", None),
    ("yawn1", "yawn1", None),
    ("yawn1_blink", "yawn1", "blink"),
    ("yawn2", "yawn2", None),
    ("sleep1", "sleep1", None),
    ("sleep2", "sleep2", None),
]
for d in ["up", "upright", "right", "downright", "down", "downleft", "left", "upleft"]:
    FRAMES += [(f"{d}1", f"{d}1", None), (f"{d}2", f"{d}2", None)]
for d in ["up", "right", "down", "left"]:
    FRAMES += [(f"{d}claw1", f"{d}claw1", None), (f"{d}claw2", f"{d}claw2", None)]

# Back views: the short vertical strokes there are ear lines, not eyes.
NO_FACE = {"up1", "up2", "upclaw1", "upclaw2"}

# Palette (RGBA).
OUTLINE = (38, 35, 44, 255)
INNER_LINE = (92, 86, 100, 255)
TIP_DARK = (104, 100, 114, 255)
TIP_MID = (146, 143, 156, 255)
TIP_LIGHT = (190, 188, 199, 255)
COAT = (226, 225, 232, 255)
CHEST = (247, 246, 250, 255)
EYE = (116, 196, 72, 255)
EYE_SHINE = (226, 250, 190, 255)
PUPIL = (22, 40, 20, 255)
NOSE = (196, 108, 104, 255)

T, O, L, F, E, N = "T", "O", "L", "F", "E", "N"


def classify(img):
    """Class map (list of rows) for a 32x32 classic sprite."""
    w = h = SRC_SIZE

    def px(x, y):
        if 0 <= x < w and 0 <= y < h:
            return img.getpixel((x, y))
        return (0, 0, 0, 0)

    def is_black(x, y):
        p = px(x, y)
        return p[3] > 0 and p[0] < 128

    def is_clear(x, y):
        return px(x, y)[3] == 0

    grid = [[T] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            p = px(x, y)
            if p[3] == 0:
                continue
            if is_black(x, y):
                touches_clear = any(
                    is_clear(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)
                )
                grid[y][x] = O if touches_clear else L
            else:
                grid[y][x] = F
    return grid


def features(grid):
    """Isolated inner-line blobs: the eyes (3px vertical) and nose (1px)."""
    inner = {(x, y) for y, row in enumerate(grid) for x, c in enumerate(row) if c == L}
    seen, comps = set(), []
    for p in sorted(inner):
        if p in seen:
            continue
        stack, comp = [p], []
        seen.add(p)
        while stack:
            q = stack.pop()
            comp.append(q)
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    r = (q[0] + dx, q[1] + dy)
                    if r in inner and r not in seen:
                        seen.add(r)
                        stack.append(r)
        touches_outline = any(
            0 <= x + dx < SRC_SIZE and 0 <= y + dy < SRC_SIZE and grid[y + dy][x + dx] == O
            for x, y in comp for dx in (-1, 0, 1) for dy in (-1, 0, 1)
        )
        if not touches_outline:
            comps.append(sorted(comp))

    eyes = [c for c in comps if len(c) == 3 and len({x for x, _ in c}) == 1]
    noses = []
    if eyes:
        eye_bottom = max(y for c in eyes for _, y in c)
        eye_xs = [c[0][0] for c in eyes]
        for c in comps:
            if len(c) == 1:
                x, y = c[0]
                if eye_bottom < y <= eye_bottom + 3 and min(eye_xs) - 1 <= x <= max(eye_xs) + 1:
                    noses.append(c)
    return eyes, noses


def scale2x(grid):
    """Scale2x (EPX) on a class map."""
    h, w = len(grid), len(grid[0])

    def at(x, y):
        return grid[min(max(y, 0), h - 1)][min(max(x, 0), w - 1)]

    out = [[T] * (w * 2) for _ in range(h * 2)]
    for y in range(h):
        for x in range(w):
            p = at(x, y)
            a, b, c, d = at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1)
            e0 = e1 = e2 = e3 = p
            if c == a and c != d and a != b:
                e0 = a
            if a == b and a != c and b != d:
                e1 = b
            if d == c and d != b and c != a:
                e2 = c
            if b == d and b != a and d != c:
                e3 = d
            # Never let smoothing eat the face features.
            if p in (E, N):
                e0 = e1 = e2 = e3 = p
            out[2 * y][2 * x] = e0
            out[2 * y][2 * x + 1] = e1
            out[2 * y + 1][2 * x] = e2
            out[2 * y + 1][2 * x + 1] = e3
    return out


def distance(grid, inside):
    """Chebyshev distance of every `inside` cell to the nearest cell that is
    not inside (outside the image counts as not inside)."""
    from collections import deque

    h, w = len(grid), len(grid[0])
    far = h + w
    dist = [[far if inside(grid[y][x]) else 0 for x in range(w)] for y in range(h)]
    queue = deque()
    for y in range(h):
        for x in range(w):
            if dist[y][x] == 0:
                queue.append((x, y))
            elif x in (0, w - 1) or y in (0, h - 1):
                dist[y][x] = 1
                queue.append((x, y))
    while queue:
        x, y = queue.popleft()
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                nx, ny = x + dx, y + dy
                if 0 <= nx < w and 0 <= ny < h and dist[ny][nx] > dist[y][x] + 1:
                    dist[ny][nx] = dist[y][x] + 1
                    queue.append((nx, ny))
    return dist


def skeleton(cells):
    """Zhang-Suen thinning of a set of (x, y) cells to 1px, 8-connected."""
    cells = set(cells)

    def ring(x, y):
        # P2..P9, clockwise from north.
        return [(x, y - 1) in cells, (x + 1, y - 1) in cells, (x + 1, y) in cells,
                (x + 1, y + 1) in cells, (x, y + 1) in cells, (x - 1, y + 1) in cells,
                (x - 1, y) in cells, (x - 1, y - 1) in cells]

    changed = True
    while changed:
        changed = False
        for step in (0, 1):
            drop = []
            for x, y in cells:
                p = ring(x, y)
                n = sum(p)
                flips = sum(1 for i in range(8) if not p[i] and p[(i + 1) % 8])
                if not (2 <= n <= 6 and flips == 1):
                    continue
                p2, p4, p6, p8 = p[0], p[2], p[4], p[6]
                if step == 0 and not (p2 and p4 and p6) and not (p4 and p6 and p8):
                    drop.append((x, y))
                if step == 1 and not (p2 and p4 and p8) and not (p2 and p6 and p8):
                    drop.append((x, y))
            if drop:
                cells.difference_update(drop)
                changed = True
    return cells


def upscale(grid):
    """Class map at SIZE. Scale2x twice keeps the shapes smooth but makes
    every 1px line 4px thick; pull the silhouette outline back to its outer
    2px, and thin the inner lines to a 1px skeleton widened to an even 2px."""
    big = grid
    while len(big) < SIZE:
        big = scale2x(big)
    h, w = len(big), len(big[0])
    to_clear = distance(big, lambda c: c != T)
    lines = [(x, y) for y in range(h) for x in range(w) if big[y][x] == L]
    out = [[F if c == L or (c == O and to_clear[y][x] > 2) else c
            for x, c in enumerate(row)] for y, row in enumerate(big)]
    for x, y in skeleton(lines):
        for dy in (0, 1):
            for dx in (0, 1):
                if 0 <= x + dx < w and 0 <= y + dy < h and out[y + dy][x + dx] == F:
                    out[y + dy][x + dx] = L
    return out


def paint(grid, name):
    h, w = len(grid), len(grid[0])
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    k = SCALE / 2  # band widths were tuned at 64px

    # Inner lines and face features sit on top of the coat, so the tipping
    # gradient runs through them; only the silhouette edge counts.
    fur = lambda c: c in (F, L, E, N)
    core = distance(grid, fur)

    # Tipping depth: fur cells since the last edge above (the back, the crown,
    # the top of the tail), averaged across neighbouring columns so the bands
    # follow the body's contour instead of streaking down single columns.
    raw = [[0] * w for _ in range(h)]
    for x in range(w):
        depth = 0
        for y in range(h):
            depth = depth + 1 if fur(grid[y][x]) else 0
            raw[y][x] = depth
    r = SCALE
    smooth = [[0.0] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            if fur(grid[y][x]):
                vals = [raw[y][xx] for xx in range(max(0, x - r), min(w, x + r + 1))
                        if fur(grid[y][xx])]
                smooth[y][x] = min(raw[y][x] + r, sum(vals) / len(vals))

    for x in range(w):
        for y in range(h):
            c = grid[y][x]
            depth = smooth[y][x]
            if c == T:
                continue
            if c == O:
                col = OUTLINE
            elif c == L:
                col = INNER_LINE
            elif c == N:
                col = NOSE
            elif c == E:
                col = EYE
            else:
                if depth <= 2 * k:
                    col = TIP_DARK
                elif depth <= 4 * k:
                    col = TIP_MID
                elif depth <= 6 * k:
                    col = TIP_LIGHT
                elif core[y][x] > 5 * k:
                    col = CHEST
                else:
                    col = COAT
            img.putpixel((x, y), col)

    # Eyes: black lids top and bottom, a dark pupil, a glint.
    lid = max(1, SCALE // 2)
    for y in range(h):
        for x in range(w):
            if grid[y][x] != E:
                continue
            up = sum(1 for d in range(1, lid + 1) if y - d >= 0 and grid[y - d][x] == E)
            down = sum(1 for d in range(1, lid + 1) if y + d < h and grid[y + d][x] == E)
            left = sum(1 for d in range(1, SCALE) if x - d >= 0 and grid[y][x - d] == E)
            if up < lid or down < lid:
                img.putpixel((x, y), OUTLINE)
            elif left in (1, 2) and SCALE >= 4 and up >= lid and down > lid:
                img.putpixel((x, y), PUPIL)
            elif left == 0 and grid[y - lid][x] == E and grid[y - lid - 1][x] != E:
                img.putpixel((x, y), EYE_SHINE)
    return img


def paint_classic(grid):
    h, w = len(grid), len(grid[0])
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    for y in range(h):
        for x in range(w):
            c = grid[y][x]
            if c != T:
                img.putpixel((x, y), (255, 255, 255, 255) if c == F else (0, 0, 0, 255))
    return img


def build_frame(src, variant, skin):
    img = Image.open(SRC / f"{src}.png").convert("RGBA")
    grid = classify(img)
    if skin == "classic":
        if variant == "blink":
            for comp in features(grid)[0]:
                for x, y in comp[::2]:
                    grid[y][x] = F
        return paint_classic(upscale(grid))
    if src not in NO_FACE:
        eyes, noses = features(grid)
        for comp in eyes:
            for x, y in comp:
                grid[y][x] = E if variant != "blink" else F
            if variant == "blink":
                # Closed eye: a short dark lid line on the middle row.
                x, y = comp[1]
                grid[y][x] = L
        for comp in noses:
            for x, y in comp:
                grid[y][x] = N
    return paint(upscale(grid), src)


def build_sheet(skin):
    rows = (len(FRAMES) + COLUMNS - 1) // COLUMNS
    sheet = Image.new("RGBA", (COLUMNS * SIZE, rows * SIZE), (0, 0, 0, 0))
    for i, (_, src, variant) in enumerate(FRAMES):
        frame = build_frame(src, variant, skin)
        sheet.paste(frame, ((i % COLUMNS) * SIZE, (i // COLUMNS) * SIZE))
    out = ASSETS / f"{skin}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out, optimize=True)
    print(f"wrote {out.relative_to(ROOT)}: {len(FRAMES)} frames, {sheet.size[0]}x{sheet.size[1]}")
    return sheet


def main():
    build_sheet("classic")
    sheet = build_sheet("burmilla")

    if "--preview" in sys.argv:
        out = Path(sys.argv[sys.argv.index("--preview") + 1])
        bg = Image.new("RGBA", sheet.size, (70, 84, 104, 255))
        bg.alpha_composite(sheet)
        bg.resize((sheet.size[0] * 2, sheet.size[1] * 2), Image.NEAREST).save(out)
        print(f"preview: {out}")


if __name__ == "__main__":
    main()
