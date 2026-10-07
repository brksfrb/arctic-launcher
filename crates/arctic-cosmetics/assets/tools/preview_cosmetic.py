"""Render a cosmetic from its files to a PNG, so you can look at it before
pushing: python tools/preview_cosmetic.py <folder> <id> [out.png]

It draws the same geometry the game draws (bones, cube rotation, per-face
texture rectangles, glow layer) from the back, a three-quarter view and the
side, next to a grey player. It is a quick check, not the game: lighting and
particles are not shown.
"""
import json
import math
import os
import sys

import numpy as np
from PIL import Image

HEIGHT = 24.0
PLAYER = [  # name, min, max (Bedrock coordinates), grey
    ("legR", (-4, 0, -2), (0, 12, 2), 95), ("legL", (0, 0, -2), (4, 12, 2), 95),
    ("body", (-4, 12, -2), (4, 24, 2), 120), ("armR", (-8, 12, -2), (-4, 24, 2), 110),
    ("armL", (4, 12, -2), (8, 24, 2), 110), ("head", (-4, 24, -4), (4, 32, 4), 140),
]


def rot(rx, ry, rz):
    sx, cx, sy, cy, sz, cz = map(float, (math.sin(rx), math.cos(rx), math.sin(ry), math.cos(ry), math.sin(rz), math.cos(rz)))
    return np.array([[cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
                     [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
                     [-sy, cy * sx, cy * cx]])


def java(p):  # Bedrock -> Java model space (y down)
    return np.array([p[0], HEIGHT - p[1], p[2]], dtype=float)


def cube_faces(c):
    """(corners tl,tr,br,bl in Java space, normal, rect or None) for the six faces."""
    o, s, inf = c["origin"], c["size"], c.get("inflate", 0)
    x0, y0, z0 = o[0] - inf, HEIGHT - o[1] - s[1] - inf, o[2] - inf
    x1, y1, z1 = x0 + s[0] + 2 * inf, y0 + s[1] + 2 * inf, z0 + s[2] + 2 * inf
    P = lambda *a: np.array(a, dtype=float)
    corners = [
        [P(x0, y0, z0), P(x1, y0, z0), P(x1, y1, z0), P(x0, y1, z0)],
        [P(x1, y0, z1), P(x0, y0, z1), P(x0, y1, z1), P(x1, y1, z1)],
        [P(x0, y0, z1), P(x0, y0, z0), P(x0, y1, z0), P(x0, y1, z1)],
        [P(x1, y0, z0), P(x1, y0, z1), P(x1, y1, z1), P(x1, y1, z0)],
        [P(x0, y0, z1), P(x1, y0, z1), P(x1, y0, z0), P(x0, y0, z0)],
        [P(x0, y1, z0), P(x1, y1, z0), P(x1, y1, z1), P(x0, y1, z1)],
    ]
    normals = [P(0, 0, -1), P(0, 0, 1), P(-1, 0, 0), P(1, 0, 0), P(0, -1, 0), P(0, 1, 0)]
    uv = c["uv"]
    if isinstance(uv, list):
        u, v = uv
        w, h, d = s
        rects = [(u + d, v + d, w, h), (u + 2 * d + w, v + d, w, h), (u, v + d, d, h), (u + d + w, v + d, d, h), (u + d, v, w, d), (u + d + w, v, w, d)]
    else:
        rects = [None if n not in uv else (*uv[n]["uv"], *uv[n]["uv_size"]) for n in ("north", "south", "east", "west", "up", "down")]
    if c.get("rotation") and any(c["rotation"]):
        R = rot(*[math.radians(a) for a in c["rotation"]])
        mid = [o[i] + s[i] / 2 for i in range(3)]
        pivot = java(c.get("pivot", mid))
        corners = [[pivot + R @ (p - pivot) for p in f] for f in corners]
        normals = [R @ n for n in normals]
    return list(zip(corners, normals, rects))


def bone_quads(geo):
    """All textured quads of the model, in the model's Java space, with the bones applied."""
    bones = {b["name"]: b for b in geo["bones"]}
    world = {}

    def place(name):
        if name in world:
            return world[name]
        b = bones[name]
        pj = java(b["pivot"])
        R = rot(*[math.radians(a) for a in b.get("rotation", (0, 0, 0))])
        if b.get("parent"):
            pR, pT, pp = place(b["parent"])
            M = pR @ R
            T = pR @ (pj - pp) + pT
        else:
            M, T = R, pj  # root: hangs at its own pivot in model space
        world[name] = (M, T, pj)
        return world[name]

    quads = []
    for name, b in bones.items():
        M, T, pj = place(name)
        for c in b.get("cubes", []):
            for corners, normal, rect in cube_faces(c):
                if rect is None:
                    continue
                pts = [M @ (p - pj) + T for p in corners]
                quads.append((pts, M @ normal, rect))
    return quads


def raster(canvas, depth, quad, tex, glow, view, scale, origin):
    pts, normal, (u, v, w, h) = quad
    th, tw = tex.shape[:2]
    scr = [view(p) for p in pts]
    xy = np.array([[origin[0] + q[0] * scale, origin[1] + q[1] * scale] for q in scr])
    z = [q[2] for q in scr]
    uvs = np.array([(u, v), (u + w, v), (u + w, v + h), (u, v + h)], dtype=float)
    for tri in ((0, 1, 2), (0, 2, 3)):
        a, b, c = (xy[i] for i in tri)
        x0, x1 = int(max(0, math.floor(min(a[0], b[0], c[0])))), int(min(canvas.shape[1] - 1, math.ceil(max(a[0], b[0], c[0]))))
        y0, y1 = int(max(0, math.floor(min(a[1], b[1], c[1])))), int(min(canvas.shape[0] - 1, math.ceil(max(a[1], b[1], c[1]))))
        if x1 < x0 or y1 < y0:
            continue
        den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(den) < 1e-9:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        l1 = ((b[1] - c[1]) * (gx - c[0]) + (c[0] - b[0]) * (gy - c[1])) / den
        l2 = ((c[1] - a[1]) * (gx - c[0]) + (a[0] - c[0]) * (gy - c[1])) / den
        l3 = 1 - l1 - l2
        inside = (l1 >= 0) & (l2 >= 0) & (l3 >= 0)
        if not inside.any():
            continue
        i0, i1, i2 = tri
        tu = l1 * uvs[i0][0] + l2 * uvs[i1][0] + l3 * uvs[i2][0]
        tv = l1 * uvs[i0][1] + l2 * uvs[i1][1] + l3 * uvs[i2][1]
        zz = l1 * z[i0] + l2 * z[i1] + l3 * z[i2]
        ix = np.clip(tu.astype(int), 0, tw - 1)
        iy = np.clip(tv.astype(int), 0, th - 1)
        texel = tex[iy, ix]
        opaque = inside & (texel[..., 3] > 25) & (zz > depth[y0:y1 + 1, x0:x1 + 1])
        shade = 0.62 + 0.38 * max(0.0, float(np.dot(normal / (np.linalg.norm(normal) + 1e-9), view_dir_light())))
        rgb = texel[..., :3].astype(float) * shade
        if glow is not None:
            g = glow[iy, ix]
            rgb = np.where((g[..., 3:4] > 25), g[..., :3].astype(float), rgb)
        region = canvas[y0:y1 + 1, x0:x1 + 1]
        region[opaque] = rgb[opaque]
        depth[y0:y1 + 1, x0:x1 + 1][opaque] = zz[opaque]


def view_dir_light():
    return np.array([0.3, -0.8, 0.5]) / np.linalg.norm([0.3, -0.8, 0.5])


def player_quads():
    quads = []
    for _, lo, hi, grey in PLAYER:
        cube = {"origin": lo, "size": [hi[i] - lo[i] for i in range(3)], "uv": {n: {"uv": [0, 0], "uv_size": [1, 1]} for n in ("north", "south", "east", "west", "up", "down")}}
        for corners, normal, rect in cube_faces(cube):
            quads.append((corners, normal, rect, grey))
    return quads


def render(folder, id, out):
    geo = json.load(open(os.path.join(folder, f"{id}.geo.json")))["minecraft:geometry"][0]
    tex = np.array(Image.open(os.path.join(folder, f"{id}.png")).convert("RGBA"))
    gpath = os.path.join(folder, f"{id}.glow.png")
    glow = np.array(Image.open(gpath).convert("RGBA")) if os.path.exists(gpath) else None
    quads = bone_quads(geo)
    grey_tex = np.full((1, 1, 4), 255, dtype=np.uint8)
    scale = 9
    # Camera azimuth around the player: 0 = behind (the wings' side), 90 = the player's left, 180 = in front.
    views = [("back", math.radians(0)), ("three-quarter", math.radians(40)), ("side", math.radians(90))]
    panels = []
    for label, yaw in views:
        c, s = math.cos(yaw), math.sin(yaw)

        def view(p):  # Java space (y down, front = -z) -> screen (x right, y down, z toward the camera)
            return np.array([-c * p[0] + s * p[2], p[1], s * p[0] + c * p[2]])

        W, H = 80 * scale, 52 * scale
        canvas = np.full((H, W, 3), 28, dtype=np.uint8)
        depth = np.full((H, W), -1e9)
        origin = (W / 2, 14 * scale)
        for corners, normal, rect, grey in player_quads():
            n = view(normal)
            if n[2] > 0:
                t = np.full((1, 1, 4), (grey, grey, grey, 255), dtype=np.uint8)
                raster(canvas, depth, (corners, normal, (0, 0, 1, 1)), t, None, view, scale, origin)
        for quad in quads:
            raster(canvas, depth, quad, tex, glow, view, scale, origin)
        panels.append(Image.fromarray(canvas))
    sheet = Image.new("RGB", (sum(p.width for p in panels) + 8 * (len(panels) - 1), panels[0].height), (60, 60, 60))
    x = 0
    for p in panels:
        sheet.paste(p, (x, 0))
        x += p.width + 8
    sheet.save(out)
    print(f"wrote {out}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    folder, id = sys.argv[1], sys.argv[2]
    render(folder, id, sys.argv[3] if len(sys.argv) > 3 else os.path.join(folder, f"{id}.preview.png"))
