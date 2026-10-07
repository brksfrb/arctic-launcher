"""Worked example: a friendly wave emote, written with the kit.

    python tools/example_emote_wave.py     # writes tools/out/wave.animation.json

An emote moves the player's own bones: head, body, rightArm, leftArm, rightLeg,
leftLeg (rotations in degrees REPLACE the walking pose of that bone; positions in
pixels are added). What makes an emote feel good rather than robotic:
  * a key pose every 0.15-0.3 s (not 3 poses over 2 s), and smooth (Catmull-Rom)
    blending, which the kit turns on by default;
  * anticipation (a small move the opposite way first) and follow-through (the
    head and body settle after the arm stops);
  * the whole body takes part: the head looks at the audience, the torso leans,
    the weight shifts (body position), the other arm and legs move a little;
  * one-shot emotes end at the rest pose (0,0,0) so there is no snap; looping
    emotes end where they start.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cosmetic_kit import Animation  # noqa: E402

a = Animation(length=2.6, loop=False)
# right arm: a small anticipation dip, up (z = +150 lifts the right arm out and up), a waving shake
# between ~125 and ~160, then back down. (The left arm lifts with NEGATIVE z.)
a.rotate("rightArm", {0: (0, 0, 0), 0.25: (0, 0, -8), 0.6: (0, 0, 140), 0.8: (0, 0, 158), 1.0: (0, 0, 126),
                       1.2: (0, 0, 158), 1.4: (0, 0, 126), 1.6: (0, 0, 156), 1.9: (0, 0, 110),
                       2.3: (0, 0, 20), 2.6: (0, 0, 0)})
a.rotate("head", {0: (0, 0, 0), 0.3: (-4, 8, 0), 0.9: (-8, 10, 4), 1.6: (-6, 6, -3), 2.2: (2, 0, 0), 2.6: (0, 0, 0)})
a.rotate("body", {0: (0, 0, 0), 0.3: (3, 0, 0), 0.9: (-2, 6, 3), 1.6: (-2, 4, -2), 2.2: (1, 0, 0), 2.6: (0, 0, 0)})
a.move("body", {0: (0, 0, 0), 0.3: (0, -0.4, 0), 0.9: (0, 0.3, 0), 1.6: (0, 0.2, 0), 2.6: (0, 0, 0)})
a.rotate("leftArm", {0: (0, 0, 0), 0.9: (6, 0, -4), 2.0: (4, 0, -3), 2.6: (0, 0, 0)})
a.rotate("rightLeg", {0: (0, 0, 0), 0.9: (-3, 0, 0), 2.6: (0, 0, 0)})
a.rotate("leftLeg", {0: (0, 0, 0), 0.9: (3, 0, 0), 2.6: (0, 0, 0)})
a.save(os.path.join(os.path.dirname(os.path.abspath(__file__)), "out", "wave.animation.json"), "wave")
print("wave: 2.6 s")
