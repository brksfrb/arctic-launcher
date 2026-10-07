"""Arctic cosmetic kit: build a 3D cosmetic (model + texture + glow + idle
animation) in code, with every convention of the format handled for you.

    from cosmetic_kit import Model, Animation, paint

    m = Model("angel_wings", slot="back", tex=(256, 128))
    body = m.bone("body", pivot=(0, 24, 0))
    wing = m.bone("wing_left", parent=body, pivot=(3, 22, 2.5))
    sprite = m.sprite(32, 16)                 # a 32x16 area of the texture
    paint.feather(sprite.image, ...)          # paint it (PIL RGBA image)
    m.card(wing, sprite, origin=(3, 14, 2.5), size=(8, 16))   # a flat painted plane
    m.save("out/")                            # writes the 3-4 files

Run `python tools/cosmetic_kit.py --selftest` to check the kit itself.

Format facts the kit relies on (see docs/content-guide.md for the full text):
  * units are pixels, 16 = one block; the player stands on y = 0, faces -z,
    +x is the player's LEFT; "up" is +y.
  * a cube's texture is one rectangle per face, in texels, so a texture can
    be much finer than the model (use 4-8 texels per unit for detail).
  * `rotation` (degrees, around `pivot`) turns a bone or a single cube, in the
    order Z, then Y, then X. A cube lying along +x with rotation z = +30 has
    its far end tipped 30 degrees DOWN; z = -30 tips it UP. A bone with
    y = -10 on the player's left side (+x) swings its tip backward.
"""
import json
import math
import os
import sys

from PIL import Image, ImageDraw

FACES = ("north", "south", "east", "west", "up", "down")
MAX_TEXTURE = 1024
MAX_CUBES = 512
MAX_BONES = 128


class Sprite:
    """A rectangle of the texture (and of the glow texture) you paint on."""

    def __init__(self, model, u, v, w, h):
        self.model, self.u, self.v, self.w, self.h = model, u, v, w, h
        self.image = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        self.glow = None  # an RGBA image the same size, only if you ask for glow

    def make_glow(self):
        if self.glow is None:
            self.glow = Image.new("RGBA", (self.w, self.h), (0, 0, 0, 0))
        return self.glow

    def rect(self, flip_x=False, flip_y=False):
        """{"uv": [u, v], "uv_size": [w, h]} (negative size flips the face)."""
        u, v, w, h = self.u, self.v, self.w, self.h
        if flip_x:
            u, w = u + w, -w
        if flip_y:
            v, h = v + h, -h
        return {"uv": [u, v], "uv_size": [w, h]}


class Model:
    def __init__(self, id, slot, tex=(128, 128)):
        assert slot in ("head", "face", "back", "body", "shoulders"), slot
        for side in tex:
            assert side & (side - 1) == 0 and 1 <= side <= MAX_TEXTURE, f"texture side {side}"
        self.id, self.slot, self.tex = id, slot, tex
        self.bones, self.sprites, self._shelf = [], [], [0, 0, 0]  # x, y, row height
        self.animation = None

    # -- structure -------------------------------------------------------
    def bone(self, name, pivot, parent=None, rotation=(0, 0, 0)):
        """A bone. Root bones are named after the player part they follow:
        head, body, rightArm, leftArm, rightLeg, leftLeg."""
        b = {"name": name, "pivot": list(pivot), "cubes": []}
        if parent is not None:
            b["parent"] = parent["name"]
        if any(rotation):
            b["rotation"] = list(rotation)
        self.bones.append(b)
        return b

    def sprite(self, w, h):
        """Reserve a w x h area of the texture (shelf packing, with a 1 texel gap)."""
        x, y, row = self._shelf
        if x + w > self.tex[0]:
            x, y, row = 0, y + row + 1, 0
        if y + h > self.tex[1]:
            raise ValueError(f"texture {self.tex} is full; use a bigger one or smaller sprites")
        s = Sprite(self, x, y, w, h)
        self._shelf = [x + w + 1, y, max(row, h)]
        self.sprites.append(s)
        return s

    # -- cubes -----------------------------------------------------------
    def cube(self, bone, origin, size, faces, rotation=None, pivot=None, inflate=0):
        """A cube with one texture rectangle per face: faces = {"north": rect, ...}
        (rect from Sprite.rect()). Faces you leave out are not drawn."""
        assert set(faces) <= set(FACES), set(faces) - set(FACES)
        c = {"origin": list(origin), "size": list(size), "uv": faces}
        if rotation and any(rotation):
            c["rotation"] = list(rotation)
            c["pivot"] = list(pivot if pivot is not None else [origin[i] + size[i] / 2 for i in range(3)])
        if inflate:
            c["inflate"] = inflate
        bone["cubes"].append(c)
        return c

    def card(self, bone, sprite, origin, size, thickness=0.25, rotation=None, pivot=None, mirror=False):
        """A flat painted plane (a thin cube showing the sprite on its back and
        front faces): `size` = (width along x, height along y). Seen from behind
        the player the picture reads normally (left edge of the sprite on the
        viewer's left); the front face shows the same picture as if the card
        were see-through. `mirror=True` flips it sideways on both faces: use it
        so a sprite painted "root at the left" lands correctly on a wing that
        grows toward +x (the player's left wing)."""
        w, h = size
        back = sprite.rect(flip_x=mirror)
        # Seen from behind, the face's left edge is +x, so the picture's left
        # edge goes to the larger x: no flip for the back face (south), flip for north.
        front = sprite.rect(flip_x=not mirror)
        return self.cube(bone, origin, (w, h, thickness), {"south": back, "north": front}, rotation, pivot)

    def box(self, bone, origin, size, sprite=None, color=None, rotation=None, pivot=None, inflate=0):
        """A plain box with one sprite per face from `sprite` (the same picture on
        all six faces) - handy for small trims. Prefer cards for anything flat."""
        s = sprite
        if s is None:
            s = self.sprite(2, 2)
            ImageDraw.Draw(s.image).rectangle([0, 0, 1, 1], fill=color or (255, 255, 255, 255))
        faces = {f: s.rect() for f in FACES}
        return self.cube(bone, origin, size, faces, rotation, pivot, inflate)

    # -- output ----------------------------------------------------------
    def geometry(self):
        return {
            "format_version": "1.12.0",
            "minecraft:geometry": [{
                "description": {"identifier": f"geometry.{self.id}", "texture_width": self.tex[0], "texture_height": self.tex[1]},
                "bones": self.bones,
            }],
        }

    def textures(self):
        base = Image.new("RGBA", self.tex, (0, 0, 0, 0))
        glow = Image.new("RGBA", self.tex, (0, 0, 0, 0))
        has_glow = False
        for s in self.sprites:
            base.paste(s.image, (s.u, s.v))
            if s.glow is not None:
                glow.paste(s.glow, (s.u, s.v))
                has_glow = True
        return base, (glow if has_glow else None)

    def check(self):
        cubes = sum(len(b["cubes"]) for b in self.bones)
        assert 1 <= len(self.bones) <= MAX_BONES, f"{len(self.bones)} bones"
        assert 1 <= cubes <= MAX_CUBES, f"{cubes} cubes (max {MAX_CUBES})"
        for b in self.bones:
            for c in b["cubes"]:
                for name, f in c["uv"].items():
                    (u, v), (w, h) = f["uv"], f["uv_size"]
                    ok = min(u, u + w) >= 0 and max(u, u + w) <= self.tex[0] and min(v, v + h) >= 0 and max(v, v + h) <= self.tex[1]
                    assert ok, f"{b['name']} face {name} reads outside the texture"
        return cubes

    def save(self, folder):
        cubes = self.check()
        os.makedirs(folder, exist_ok=True)
        with open(os.path.join(folder, f"{self.id}.geo.json"), "w") as f:
            json.dump(self.geometry(), f, indent=1)
        base, glow = self.textures()
        base.save(os.path.join(folder, f"{self.id}.png"))
        if glow is not None:
            glow.save(os.path.join(folder, f"{self.id}.glow.png"))
        if self.animation is not None:
            with open(os.path.join(folder, f"{self.id}.animation.json"), "w") as f:
                json.dump(self.animation.json(self.id), f, indent=1)
        print(f"{self.id}: {len(self.bones)} bones, {cubes} cubes, texture {self.tex[0]}x{self.tex[1]}"
              f"{', glow' if glow is not None else ''}{', animated' if self.animation else ''}")


class Animation:
    """One looping idle animation (cosmetics) or one emote (whole player).

        a = Animation(length=2.0, loop=True)
        a.rotate("wing_left", {0: (0, 0, 0), 1.0: (0, 25, 0), 2.0: (0, 0, 0)})

    Keyframes blend smoothly (Catmull-Rom) by default; pass smooth=False for
    straight lines. Rotations are degrees, positions are pixels.
    """

    def __init__(self, length, loop=True):
        assert 0 < length <= 15
        self.length, self.loop, self.bones = length, loop, {}

    def _channel(self, bone, kind, frames, smooth):
        track = {}
        for t, value in sorted(frames.items()):
            assert 0 <= t <= self.length, f"keyframe {t}s is outside the animation"
            v = [round(float(x), 3) for x in value]
            track[f"{t:g}"] = {"post": v, "lerp_mode": "catmullrom"} if smooth else v
        self.bones.setdefault(bone, {})[kind] = track

    def rotate(self, bone, frames, smooth=True):
        self._channel(bone, "rotation", frames, smooth)

    def move(self, bone, frames, smooth=True):
        self._channel(bone, "position", frames, smooth)

    def json(self, name):
        return {"format_version": "1.8.0", "animations": {f"animation.{name}": {
            "loop": self.loop, "animation_length": self.length, "bones": self.bones}}}

    def save(self, path, name):
        """Write an emote file (emotes/<id>.animation.json)."""
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, "w") as f:
            json.dump(self.json(name), f, indent=1)


class paint:
    """Painting helpers (all take/return RGBA PIL images or tuples)."""

    @staticmethod
    def shade(rgba, k):
        r, g, b, a = rgba
        return (max(0, min(255, int(r * k))), max(0, min(255, int(g * k))), max(0, min(255, int(b * k))), a)

    @staticmethod
    def mix(a, b, t):
        return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(4))

    @staticmethod
    def outline(img, color):
        """A 1 texel outline around everything opaque (clean silhouette)."""
        w, h = img.size
        src = img.load()
        out = img.copy()
        px = out.load()
        for y in range(h):
            for x in range(w):
                if src[x, y][3] == 0:
                    near = any(0 <= x + dx < w and 0 <= y + dy < h and src[x + dx, y + dy][3] > 0
                               for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
                    if near:
                        px[x, y] = color
        return out

    @staticmethod
    def feather(w, h, base, tip, edge, vein=None, glow_tip=None):
        """A feather lying along +x (root at the left, tip at the right), drawn in a
        w x h image: a pointed leaf shape, a gradient from `base` to `tip`, barbs
        (diagonal lines) and a center vein. Returns (image, glow_image or None)."""
        img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        px = img.load()
        mid = (h - 1) / 2
        vein = vein or paint.shade(base, 1.25)
        for x in range(w):
            t = x / (w - 1)
            # half height: grows fast, stays full, then narrows to a point
            half = mid * min(1.0, t * 5 + 0.15) * (1 - max(0.0, (t - 0.55) / 0.45) ** 1.6)
            for y in range(h):
                d = abs(y - mid)
                if d <= half:
                    col = paint.mix(base, tip, t ** 1.3)
                    # barbs: diagonal stripes, lighter on the upper side
                    stripe = ((x + (y if y < mid else -y)) // 2) % 2
                    col = paint.shade(col, 1.08 if stripe else 0.94)
                    col = paint.shade(col, 1.12 - 0.3 * (d / max(half, 1)))
                    px[x, y] = col
            ym = int(round(mid))
            if x > 1 and px[x, ym][3] > 0:
                px[x, ym] = vein
        img = paint.outline(img, edge)
        glow = None
        if glow_tip:
            glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
            gp, ip = glow.load(), img.load()
            for x in range(int(w * 0.7), w):
                for y in range(h):
                    if ip[x, y][3] > 0 and abs(y - mid) <= 1:
                        gp[x, y] = glow_tip
        return img, glow


if __name__ == "__main__" and "--selftest" in sys.argv:
    m = Model("selftest", "back", tex=(64, 64))
    b = m.bone("body", (0, 24, 0))
    s = m.sprite(16, 8)
    s.image.paste((255, 0, 0, 255), (0, 0, 16, 8))
    m.card(b, s, origin=(-8, 12, 2), size=(16, 8))
    m.box(b, (-1, 20, 2), (2, 2, 2), color=(0, 255, 0, 255))
    a = Animation(2.0)
    a.rotate("body", {0: (0, 0, 0), 1: (0, 10, 0), 2: (0, 0, 0)})
    m.animation = a
    m.save(os.path.join(os.environ.get("TEMP", "/tmp"), "kit_selftest"))
    print("ok")
