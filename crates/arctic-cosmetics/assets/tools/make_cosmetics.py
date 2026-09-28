"""Generate Arctic's built-in 3D cosmetics and emotes (Bedrock geometry, box UV).

Each cosmetic is a few boxes on the player's bones plus a painted texture:
the boxes' UV areas are packed into the texture automatically and painted
from each box's colors, with simple shading so the faces read in 3D.

Run from anywhere:  python crates/arctic-cosmetics/assets/tools/make_cosmetics.py
It rewrites the generated files in cosmetics/ and emotes/ and the generated
entries in cosmetics.json / emotes.json (hand-made ones are kept).
"""
import json
import os
import random

from PIL import Image

ASSETS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PIVOT = {"head": [0, 24, 0], "body": [0, 24, 0], "rightArm": [-5, 22, 0], "leftArm": [5, 22, 0]}


def hexcolor(c):
    c = c.lstrip("#")
    return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4)) + (255,)


def shade(rgba, k):
    r, g, b, a = rgba
    return (max(0, min(255, int(r * k))), max(0, min(255, int(g * k))), max(0, min(255, int(b * k))), a)


class Box:
    """A cube: origin/size in model units (integers), a color, and optional
    per-face painters: face -> fn(x, y, w, h) returning a color or None."""

    def __init__(self, origin, size, color, faces=None, rotation=None):
        self.origin = origin
        self.size = size
        self.color = hexcolor(color)
        self.faces = faces or {}
        self.uv = None


class Bone:
    def __init__(self, name, parent, boxes, pivot=None, rotation=None):
        self.name = name
        self.parent = parent
        self.boxes = boxes
        self.pivot = pivot
        self.rotation = rotation


def uv_size(box):
    w, h, d = box.size
    return 2 * (w + d), d + h


def pack(boxes, width):
    """Shelf-pack box UV areas; returns the texture height (a power of two)."""
    x = y = shelf = 0
    for b in sorted(boxes, key=lambda b: -uv_size(b)[1]):
        bw, bh = uv_size(b)
        if x + bw > width:
            x, y, shelf = 0, y + shelf, 0
        b.uv = [x, y]
        x += bw
        shelf = max(shelf, bh)
    height = 16
    while height < y + shelf:
        height *= 2
    return height


def paint(boxes, width, height, seed):
    rnd = random.Random(seed)
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    px = img.load()
    for b in boxes:
        w, h, d = b.size
        u, v = b.uv
        # Box UV faces: (name, x, y, width, height, light)
        faces = [
            ("top", u + d, v, w, d, 1.18),
            ("bottom", u + d + w, v, w, d, 0.72),
            ("east", u, v + d, d, h, 0.9),
            ("north", u + d, v + d, w, h, 1.0),
            ("west", u + d + w, v + d, d, h, 0.9),
            ("south", u + 2 * d + w, v + d, w, h, 0.95),
        ]
        for name, fx, fy, fw, fh, light in faces:
            painter = b.faces.get(name) or b.faces.get("all")
            for yy in range(fh):
                for xx in range(fw):
                    c = painter(xx, yy, fw, fh) if painter else None
                    base = hexcolor(c) if c else b.color
                    k = light * (1.0 + rnd.uniform(-0.05, 0.05))
                    # A darker rim reads as edges at small sizes.
                    if fw > 2 and fh > 2 and (xx in (0, fw - 1) or yy in (0, fh - 1)):
                        k *= 0.9
                    px[fx + xx, fy + yy] = shade(base, k)
    return img


def write_model(cid, bones, width=64):
    boxes = [b for bone in bones for b in bone.boxes]
    height = pack(boxes, width)
    out_bones = []
    for bone in bones:
        entry = {"name": bone.name, "pivot": bone.pivot or PIVOT.get(bone.name, [0, 24, 0])}
        if bone.parent:
            entry["parent"] = bone.parent
        if bone.rotation:
            entry["rotation"] = bone.rotation
        if bone.boxes:
            entry["cubes"] = [{"origin": b.origin, "size": b.size, "uv": b.uv} for b in bone.boxes]
        out_bones.append(entry)
    geo = {
        "format_version": "1.12.0",
        "minecraft:geometry": [{
            "description": {"identifier": f"geometry.{cid}", "texture_width": width, "texture_height": height},
            "bones": out_bones,
        }],
    }
    with open(os.path.join(ASSETS, "cosmetics", f"{cid}.geo.json"), "w", encoding="utf-8") as f:
        json.dump(geo, f, indent=1)
    paint(boxes, width, height, cid).save(os.path.join(ASSETS, "cosmetics", f"{cid}.png"))


def write_animation(path, name, length, bones, loop):
    anim = {"format_version": "1.8.0", "animations": {name: {"loop": loop, "animation_length": length, "bones": bones}}}
    with open(path, "w", encoding="utf-8") as f:
        json.dump(anim, f, indent=1)


def stripes(colors, horizontal=True, width=1):
    return lambda x, y, w, h: colors[((y if horizontal else x) // width) % len(colors)]


def band(color, rows):
    return lambda x, y, w, h: color if y in rows else None


# ---- Cosmetics ---------------------------------------------------------------------

COSMETICS = []


def cosmetic(cid, name, slot, bones, anim=None, width=64):
    write_model(cid, bones, width)
    if anim:
        write_animation(os.path.join(ASSETS, "cosmetics", f"{cid}.animation.json"), f"animation.{cid}.idle", *anim)
    COSMETICS.append({"id": cid, "name": name, "slot": slot})


cosmetic("top_hat", "Top Hat", "head", [
    Bone("head", None, [
        Box([-5, 32, -5], [10, 1, 10], "#1c1c22"),
        Box([-3, 33, -3], [6, 7, 6], "#22222a", {"east": band("#b91c1c", [5]), "north": band("#b91c1c", [5]),
                                                 "west": band("#b91c1c", [5]), "south": band("#b91c1c", [5])}),
    ]),
])

gem = {"north": lambda x, y, w, h: "#dc2626" if (x == w // 2 and y == 0) else None}
cosmetic("crown", "Crown", "head", [
    Bone("head", None, [
        Box([-4, 32, -5], [8, 2, 1], "#eab308", {"north": lambda x, y, w, h: "#2563eb" if x in (1, w - 2) and y == 1 else ("#dc2626" if x in (3, 4) and y == 0 else None)}),
        Box([-4, 32, 4], [8, 2, 1], "#eab308"),
        Box([4, 32, -4], [1, 2, 8], "#eab308"),
        Box([-5, 32, -4], [1, 2, 8], "#eab308"),
        Box([-4, 34, -5], [1, 2, 1], "#facc15"),
        Box([-1, 34, -5], [2, 3, 1], "#facc15", gem),
        Box([3, 34, -5], [1, 2, 1], "#facc15"),
        Box([-4, 34, 4], [1, 2, 1], "#facc15"),
        Box([-1, 34, 4], [2, 2, 1], "#facc15"),
        Box([3, 34, 4], [1, 2, 1], "#facc15"),
    ]),
])

cosmetic("party_hat", "Party Hat", "head", [
    Bone("head", None, [
        Box([-3, 32, -3], [6, 2, 6], "#ec4899", {"all": stripes(["#ec4899", "#f9a8d4"], horizontal=False)}),
        Box([-2, 34, -2], [4, 2, 4], "#3b82f6", {"all": stripes(["#3b82f6", "#93c5fd"], horizontal=False)}),
        Box([-1, 36, -1], [2, 2, 2], "#facc15"),
        Box([-1, 38, -1], [2, 1, 2], "#f8fafc"),
    ]),
])

inner_pink = {"north": lambda x, y, w, h: "#f9a8d4" if 0 < x < w - 1 and y > 0 else None}
cosmetic("cat_ears", "Cat Ears", "head", [
    Bone("head", None, [
        Box([1, 32, -2], [3, 2, 1], "#f97316", inner_pink),
        Box([2, 34, -2], [1, 1, 1], "#f97316"),
        Box([-4, 32, -2], [3, 2, 1], "#f97316", inner_pink),
        Box([-3, 34, -2], [1, 1, 1], "#f97316"),
    ]),
])

bunny_inner = {"north": lambda x, y, w, h: "#fbcfe8" if y > 0 and y < h - 1 else None}
cosmetic("bunny_ears", "Bunny Ears", "head", [
    Bone("head", None, []),
    Bone("ear_left", "head", [Box([1, 32, -1], [2, 7, 1], "#f8fafc", bunny_inner)], pivot=[2, 32, 0], rotation=[0, 0, -12]),
    Bone("ear_right", "head", [Box([-3, 32, -1], [2, 7, 1], "#f8fafc", bunny_inner)], pivot=[-2, 32, 0], rotation=[0, 0, 12]),
])

cosmetic("viking_helmet", "Viking Helmet", "head", [
    Bone("head", None, [
        Box([-4, 32, -4], [8, 1, 8], "#6b7280"),
        Box([-5, 29, -5], [10, 3, 1], "#6b7280", {"north": band("#9ca3af", [0])}),
        Box([-5, 29, 4], [10, 3, 1], "#6b7280"),
        Box([4, 29, -4], [1, 3, 8], "#6b7280"),
        Box([-5, 29, -4], [1, 3, 8], "#6b7280"),
        Box([5, 31, -1], [2, 2, 2], "#f5f0e1"),
        Box([6, 33, -1], [1, 3, 1], "#f5f0e1"),
        Box([-7, 31, -1], [2, 2, 2], "#f5f0e1"),
        Box([-7, 33, -1], [1, 3, 1], "#f5f0e1"),
    ]),
])

cosmetic("santa_hat", "Santa Hat", "head", [
    Bone("head", None, [
        Box([-5, 31, -5], [10, 2, 10], "#f8fafc"),
        Box([-4, 33, -4], [8, 3, 8], "#dc2626"),
    ]),
    Bone("hat_tip", "head", [
        Box([-2, 36, -2], [5, 2, 5], "#dc2626"),
        Box([0, 38, 0], [3, 2, 3], "#b91c1c"),
        Box([2, 39, 2], [2, 2, 2], "#f8fafc"),
    ], pivot=[0, 36, 0], rotation=[-12, 0, 0]),
])

cosmetic("headphones", "Headphones", "head", [
    Bone("head", None, [
        Box([-5, 32, -1], [10, 1, 2], "#27272a"),
        Box([-5, 29, -1], [1, 3, 2], "#27272a"),
        Box([4, 29, -1], [1, 3, 2], "#27272a"),
        Box([-6, 26, -2], [2, 4, 4], "#18181b", {"east": lambda x, y, w, h: "#38bdf8" if 0 < x < w - 1 and 0 < y < h - 1 else None}),
        Box([4, 26, -2], [2, 4, 4], "#18181b", {"west": lambda x, y, w, h: "#38bdf8" if 0 < x < w - 1 and 0 < y < h - 1 else None}),
    ]),
])

glint = {"north": lambda x, y, w, h: "#e5e7eb" if (x, y) == (w - 1, 0) else None}
cosmetic("sunglasses", "Sunglasses", "face", [
    Bone("head", None, [
        Box([1, 27, -5], [3, 2, 1], "#0a0a0a", glint),
        Box([-4, 27, -5], [3, 2, 1], "#0a0a0a", glint),
        Box([-1, 28, -5], [2, 1, 1], "#0a0a0a"),
        Box([4, 28, -4], [1, 1, 4], "#0a0a0a"),
        Box([-5, 28, -4], [1, 1, 4], "#0a0a0a"),
    ]),
])

cosmetic("glasses_3d", "3D Glasses", "face", [
    Bone("head", None, [
        Box([1, 27, -5], [3, 2, 1], "#ef4444"),
        Box([-4, 27, -5], [3, 2, 1], "#22d3ee"),
        Box([-1, 28, -5], [2, 1, 1], "#f8fafc"),
        Box([4, 28, -4], [1, 1, 4], "#f8fafc"),
        Box([-5, 28, -4], [1, 1, 4], "#f8fafc"),
    ]),
])

cosmetic("mustache", "Mustache", "face", [
    Bone("head", None, [
        Box([-2, 26, -5], [4, 1, 1], "#3f2a1d"),
        Box([-3, 25, -5], [2, 1, 1], "#3f2a1d"),
        Box([1, 25, -5], [2, 1, 1], "#3f2a1d"),
    ]),
])

cosmetic("backpack", "Backpack", "back", [
    Bone("body", None, [
        Box([-3, 13, 2], [6, 9, 3], "#92400e", {"top": lambda x, y, w, h: "#b45309"}),
        Box([-3, 20, 5], [6, 2, 1], "#b45309"),
        Box([-2, 14, 5], [4, 4, 1], "#78350f", {"south": lambda x, y, w, h: "#fbbf24" if (x, y) == (w // 2, 0) else None}),
    ]),
])

membrane = {"all": lambda x, y, w, h: "#a78bfa" if (x + y) % 4 else "#7c3aed"}
cosmetic("dragon_wings", "Dragon Wings", "back", [
    Bone("body", None, []),
    Bone("wing_left", "body", [
        Box([2, 21, 2], [14, 2, 1], "#4c1d95"),
        Box([2, 11, 2], [12, 10, 1], "#8b5cf6", membrane),
        Box([14, 13, 2], [2, 8, 1], "#4c1d95"),
    ], pivot=[2, 22, 2], rotation=[0, -25, 0]),
    Bone("wing_right", "body", [
        Box([-16, 21, 2], [14, 2, 1], "#4c1d95"),
        Box([-14, 11, 2], [12, 10, 1], "#8b5cf6", membrane),
        Box([-16, 13, 2], [2, 8, 1], "#4c1d95"),
    ], pivot=[-2, 22, 2], rotation=[0, 25, 0]),
], anim=(1.6, {
    "wing_left": {"rotation": {"0.0": [0, 0, 0], "0.8": [0, -28, 0], "1.6": [0, 0, 0]}},
    "wing_right": {"rotation": {"0.0": [0, 0, 0], "0.8": [0, 28, 0], "1.6": [0, 0, 0]}},
}, True))

slime_face = {"north": lambda x, y, w, h: "#14532d" if (y == 1 and x in (0, w - 1)) or (y == h - 1 and 0 < x < w - 1) else None}
cosmetic("shoulder_slime", "Shoulder Slime", "shoulders", [
    Bone("body", None, []),
    Bone("slime", "body", [
        Box([4, 24, -2], [4, 4, 4], "#4ade80", slime_face),
        Box([5, 25, -1], [2, 2, 2], "#16a34a"),
    ], pivot=[6, 24, 0]),
], anim=(1.2, {"slime": {"position": {"0.0": [0, 0, 0], "0.3": [0, 0.6, 0], "0.6": [0, 0, 0], "1.2": [0, 0, 0]}}}, True))

scarf = {"all": stripes(["#dc2626", "#dc2626", "#f8fafc"], horizontal=False)}
cosmetic("scarf", "Scarf", "body", [
    Bone("body", None, [
        Box([-4, 22, -3], [8, 2, 1], "#dc2626", scarf),
        Box([-4, 22, 2], [8, 2, 1], "#dc2626", scarf),
        Box([4, 22, -3], [1, 2, 6], "#dc2626"),
        Box([-5, 22, -3], [1, 2, 6], "#dc2626"),
        Box([1, 16, -4], [2, 6, 1], "#dc2626", {"all": stripes(["#dc2626", "#dc2626", "#f8fafc"])}),
    ]),
])

# ---- Emotes ------------------------------------------------------------------------

EMOTES = []


def emote(eid, name, length, bones, loop=False):
    write_animation(os.path.join(ASSETS, "emotes", f"{eid}.animation.json"), f"animation.{eid}", length, bones, loop)
    EMOTES.append({"id": eid, "name": name})


emote("bow", "Bow", 1.6, {
    "body": {"rotation": {"0.0": [0, 0, 0], "0.3": [35, 0, 0], "1.1": [35, 0, 0], "1.5": [0, 0, 0]}},
    "head": {"rotation": {"0.0": [0, 0, 0], "0.3": [20, 0, 0], "1.1": [20, 0, 0], "1.5": [0, 0, 0]}},
    "rightArm": {"rotation": {"0.0": [0, 0, 0], "0.3": [-40, 0, 0], "1.1": [-40, 0, 0], "1.5": [0, 0, 0]}},
})

clap = {}
for i in range(7):
    t = round(0.2 + i * 0.2, 2)
    clap[str(t)] = [-80, 0, 18 if i % 2 else 4]
emote("clap", "Clap", 1.8, {
    "rightArm": {"rotation": {"0.0": [0, 0, 0], **clap, "1.8": [0, 0, 0]}},
    "leftArm": {"rotation": {"0.0": [0, 0, 0], **{k: [v[0], 0, -v[2]] for k, v in clap.items()}, "1.8": [0, 0, 0]}},
})

emote("cheer", "Cheer", 2.0, {
    "rightArm": {"rotation": {"0.0": [0, 0, 0], "0.25": [0, 0, 160], "0.5": [0, 0, 140], "0.75": [0, 0, 160], "1.0": [0, 0, 140], "1.25": [0, 0, 160], "1.8": [0, 0, 0]}},
    "leftArm": {"rotation": {"0.0": [0, 0, 0], "0.25": [0, 0, -160], "0.5": [0, 0, -140], "0.75": [0, 0, -160], "1.0": [0, 0, -140], "1.25": [0, 0, -160], "1.8": [0, 0, 0]}},
    "head": {"rotation": {"0.0": [0, 0, 0], "0.3": [-15, 0, 0], "1.5": [-15, 0, 0], "1.8": [0, 0, 0]}},
})

emote("dance", "Dance", 1.0, {
    "body": {"rotation": {"0.0": [0, 0, -6], "0.5": [0, 0, 6], "1.0": [0, 0, -6]}},
    "head": {"rotation": {"0.0": [0, 0, 8], "0.25": [-8, 0, 0], "0.5": [0, 0, -8], "0.75": [-8, 0, 0], "1.0": [0, 0, 8]}},
    "rightArm": {"rotation": {"0.0": [-30, 0, 30], "0.5": [30, 0, 10], "1.0": [-30, 0, 30]}},
    "leftArm": {"rotation": {"0.0": [30, 0, -10], "0.5": [-30, 0, -30], "1.0": [30, 0, -10]}},
    "rightLeg": {"rotation": {"0.0": [0, 0, 0], "0.25": [-20, 0, 0], "0.5": [0, 0, 0], "1.0": [0, 0, 0]}},
    "leftLeg": {"rotation": {"0.0": [0, 0, 0], "0.5": [0, 0, 0], "0.75": [-20, 0, 0], "1.0": [0, 0, 0]}},
}, loop=True)

emote("facepalm", "Facepalm", 2.4, {
    "rightArm": {"rotation": {"0.0": [0, 0, 0], "0.35": [-120, 0, -35], "2.0": [-120, 0, -35], "2.4": [0, 0, 0]}},
    "head": {"rotation": {"0.0": [0, 0, 0], "0.35": [25, 0, 0], "2.0": [25, 0, 0], "2.4": [0, 0, 0]}},
})


def merge(path, generated):
    """Keep hand-made entries; replace generated ones (by id), in order."""
    with open(path, encoding="utf-8") as f:
        existing = json.load(f)
    ids = {g["id"] for g in generated}
    kept = [e for e in existing if e["id"] not in ids]
    with open(path, "w", encoding="utf-8") as f:
        f.write("[\n" + ",\n".join("  " + json.dumps(e) for e in kept + generated) + "\n]\n")


merge(os.path.join(ASSETS, "cosmetics.json"), COSMETICS)
merge(os.path.join(ASSETS, "emotes.json"), EMOTES)
print(f"{len(COSMETICS)} cosmetics, {len(EMOTES)} emotes")
