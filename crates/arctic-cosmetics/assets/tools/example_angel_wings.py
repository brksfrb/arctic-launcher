"""Worked example: angel wings made of painted feather cards, with a glow and a
gentle flap. Copy this file, change the colors, counts and lengths, and you have
a new pair of wings.

    python tools/example_angel_wings.py            # writes tools/out/angel_wings/*
    python tools/preview_cosmetic.py tools/out/angel_wings angel_wings

Why it looks good (the recipe, in order of importance):
  1. Feathers are flat cards with a painted, alpha-cut silhouette, not boxes.
  2. The texture is 8 texels per model unit: a 28-unit feather is 224 texels long.
  3. Three rows (long primaries, shorter secondaries, small coverts) fan out at
     different angles and sit a hair apart in z so they layer without z-fighting.
  4. The tips glow (angel_wings.glow.png is drawn fullbright).
  5. The wing flaps with a smooth (Catmull-Rom) loop; feathers lag the shoulder.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cosmetic_kit import Animation, Model, paint  # noqa: E402

DENSITY = 8  # texels per model unit

# (feathers, length range, width, first angle, last angle, z, base color, tip color)
ROWS = [
    # primaries: long, the lowest row, fanning from nearly level to hanging down
    (9, (30, 21), 6.5, -12, 78, 2.30, (246, 244, 255, 255), (178, 205, 255, 255)),
    # secondaries
    (8, (22, 14), 5.5, -24, 70, 2.55, (255, 255, 255, 255), (200, 224, 255, 255)),
    # coverts: small, near the shoulder
    (6, (14, 8), 4.5, -30, 60, 2.80, (255, 252, 240, 255), (230, 240, 255, 255)),
]
EDGE = (96, 120, 176, 255)
GLOW = (130, 235, 255, 255)
ROOT = (2.6, 21.0)  # shoulder blade: x and y of the feathers' roots


def build():
    m = Model("angel_wings", slot="back", tex=(256, 256))
    body = m.bone("body", pivot=(0, 24, 0))
    wings = {}
    for side, sign in (("left", 1), ("right", -1)):
        wings[side] = m.bone(f"wing_{side}", parent=body, pivot=(sign * ROOT[0], ROOT[1], 2.5))

    sprites = []
    for i, (count, (long_len, short_len), width, a0, a1, z, base, tip) in enumerate(ROWS):
        w, h = int(long_len * DENSITY), int(width * DENSITY)
        sprite = m.sprite(w, h)
        img, glow = paint.feather(w, h, base, tip, EDGE, glow_tip=GLOW if i < 2 else None)
        sprite.image.paste(img, (0, 0))
        if glow is not None:
            sprite.make_glow().paste(glow, (0, 0))
        sprites.append(sprite)

    for side, sign in (("left", 1), ("right", -1)):
        bone = wings[side]
        for i, (count, (long_len, short_len), width, a0, a1, z, base, tip) in enumerate(ROWS):
            for k in range(count):
                t = k / (count - 1)
                length = long_len + (short_len - long_len) * t
                # angle: positive tips the feather DOWN (toward the player's feet)
                angle = a0 + (a1 - a0) * t
                x0 = sign * ROOT[0] if sign > 0 else sign * ROOT[0] - length
                origin = (x0, ROOT[1] - width / 2, z)
                # the card spans x0 .. x0+length; rotate it around the shoulder root
                # (for the right wing the feather points toward -x, so the turn flips)
                rot = (0, 0, sign * angle)
                m.card(bone, sprites[i], origin=origin, size=(length, width), rotation=rot,
                       pivot=(sign * ROOT[0], ROOT[1], z), mirror=(sign > 0))

    flap = Animation(length=3.2, loop=True)
    for side, sign in (("left", 1), ("right", -1)):
        flap.rotate(f"wing_{side}", {0: (0, sign * -6, sign * 0), 0.8: (0, sign * -10, sign * -9), 1.6: (0, sign * -6, sign * 2),
                                      2.4: (0, sign * -10, sign * -7), 3.2: (0, sign * -6, sign * 0)})
    m.animation = flap
    return m


if __name__ == "__main__":
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "out", "angel_wings")
    build().save(out)
