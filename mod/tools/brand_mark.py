"""The official Arctic snowflake raster (same pixels as BrandMark.java / icon_raster.rs)."""
import math
from PIL import Image

PI, TAU = math.pi, 2 * math.pi
INNER = (196.0, 234.0, 254.0)
OUTER = (14.0, 165.0, 233.0)
SS = 4
KITE = [(0.0, -0.46), (0.075, -0.22), (0.0, -0.10), (-0.075, -0.22)]
NOTCH = [
    (0.0, -0.36),
    (0.10, -0.42),
    (0.10, -0.30),
    (0.0, -0.27),
    (-0.10, -0.30),
    (-0.10, -0.42),
]
BARBS = [(0.04, -0.33, 0.13, -0.38), (-0.04, -0.33, -0.13, -0.38)]
BARB_WIDTH = 0.05
CORE_RADIUS = 0.10
GRADIENT_RADIUS = 0.46
MAX_SIZE = 1024

_CACHE: dict[int, Image.Image] = {}


def point_in_polygon(x, y, poly):
    inside = False
    j = len(poly) - 1
    for i, (xi, yi) in enumerate(poly):
        xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / ((yj - yi) or 1e-12) + xi:
            inside = not inside
        j = i
    return inside


def dist_to_segment(px, py, ax, ay, bx, by):
    abx, aby = bx - ax, by - ay
    len_sq = max(abx * abx + aby * aby, 1e-12)
    t = max(0.0, min(1.0, ((px - ax) * abx + (py - ay) * aby) / len_sq))
    return math.hypot(ax + abx * t - px, ay + aby * t - py)


def inside_hexagon(u, v, r):
    corners = [(r * math.cos(k * PI / 3.0), r * math.sin(k * PI / 3.0)) for k in range(6)]
    return point_in_polygon(u, v, corners)


def inside_mark(u, v):
    if inside_hexagon(u, v, CORE_RADIUS):
        return True
    for k in range(6):
        a = -k * TAU / 6.0
        s, c = math.sin(a), math.cos(a)
        lu, lv = u * c - v * s, u * s + v * c
        blade = point_in_polygon(lu, lv, KITE) and not point_in_polygon(lu, lv, NOTCH)
        barb = any(
            dist_to_segment(lu, lv, x0, y0, x1, y1) <= BARB_WIDTH / 2.0
            for x0, y0, x1, y1 in BARBS
        )
        if blade or barb:
            return True
    return False


def clamp_size(size: int) -> int:
    return max(1, min(MAX_SIZE, int(size)))


def native(size: int) -> Image.Image:
    size = clamp_size(size)
    if size in _CACHE:
        return _CACHE[size].copy()
    n = float(size)
    buf = bytearray(size * size * 4)
    i = 0
    for y in range(size):
        for x in range(size):
            hits = 0
            for sy in range(SS):
                for sx in range(SS):
                    u = (x + (sx + 0.5) / SS) / n - 0.5
                    v = (y + (sy + 0.5) / SS) / n - 0.5
                    if inside_mark(u, v):
                        hits += 1
            coverage = hits / (SS * SS)
            cx = (x + 0.5) / n - 0.5
            cy = (y + 0.5) / n - 0.5
            t = min(1.0, max(0.0, math.hypot(cx, cy) / GRADIENT_RADIUS))
            r = INNER[0] + (OUTER[0] - INNER[0]) * t
            g = INNER[1] + (OUTER[1] - INNER[1]) * t
            b = INNER[2] + (OUTER[2] - INNER[2]) * t
            buf[i : i + 4] = bytes((int(r), int(g), int(b), int(coverage * 255)))
            i += 4
    im = Image.frombytes("RGBA", (size, size), bytes(buf))
    _CACHE[size] = im
    return im.copy()
