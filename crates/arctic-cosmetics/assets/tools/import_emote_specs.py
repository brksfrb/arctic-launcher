"""Turn reviewed emote studies (docs/requests/emote-art-pack/**/motion-spec.json) into game emotes.

    python tools/import_emote_specs.py            # writes emotes/<id>.animation.json, updates emotes.json
    python tools/import_emote_specs.py --check    # only reports what would change

The studies are design data: per-part rotations (degrees, in the game's model-part convention:
the right arm lifts out and up with +z, a part pitches down/forward with +x), whole-body
lift/shift (pixels) and turn (yaw degrees), and for some hands a target point (Blockbench
pixels: y up from the ground, the player's right is -x, the front is -z) instead of an angle.

They become emotes for the client's expressive rig (mod/core/.../looks/Rig.java): bones root
(drawn only: [0, turn, 0] and [shift, lift, 0]), torso (bends at the waist and carries the head
and arms), head, rightArm, leftArm, rightLeg, leftLeg. Every channel is sampled at 30 fps with
an ease-in-out between the study's keys (holds stay still, impacts stay sharp, nothing
overshoots), then keys a straight line already reproduces are dropped. Hand targets are turned
into arm angles here (the arm stays one straight part); seated emotes get their height from the
lowest foot corner.

Studies that need joints the rig doesn't have yet (elbows, knees, a separate pelvis) are skipped
and listed.
"""
import glob
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.dirname(HERE)
REPO = os.path.abspath(os.path.join(ASSETS, "..", "..", ".."))
SPECS = os.path.join(REPO, "docs", "requests", "emote-art-pack")

FPS = 30
# Keys within this of a straight line between their neighbours are dropped (degrees / pixels).
TOLERANCE = 0.05

# Study folder -> (emote id, name). Ids already in the catalog (wave, bow, clap, facepalm) are
# deliberately not reused: these are new, separate emotes.
EMOTES = {
    "hmm": ("hmm", "Hmm"),
    "heartfelt-thanks": ("heartfelt_thanks", "Heartfelt Thanks"),
    "achoo": ("achoo", "Achoo!"),
    "zen-hover": ("zen_hover", "Zen Hover"),
    "twerk": ("twerk", "Twerk"),
    "hello-wave": ("hello_wave", "Hello Wave"),
    "respect-bow": ("respect_bow", "Respect Bow"),
    "triple-clap": ("triple_clap", "Triple Clap"),
    "no-way": ("no_way", "No Way"),
    "over-there": ("over_there", "Over There"),
    "quick-salute": ("quick_salute", "Quick Salute"),
    "side-shuffle": ("side_shuffle", "Side Shuffle"),
    "wake-up-stretch": ("wake_up_stretch", "Wake-up Stretch"),
    "showboat-spin": ("showboat_spin", "Showboat Spin"),
    "take-a-seat": ("take_a_seat", "Take a Seat"),
    "facepalm-act": ("facepalm_act", "Facepalm Act"),
    "victory-hop-emote": ("victory_hop", "Victory Hop"),
}
# Tracks only the expressive rig's later joints can play.
NEEDS_JOINTS = set()

# The game's player model (pixels, y down from the neck, front -z).
SHOULDER = {"right": (-5.0, 2.0, 0.0), "left": (5.0, 2.0, 0.0)}
# The middle of the hand, from the shoulder pivot, with the arm hanging down.
HAND = {"right": (-1.0, 10.0, 0.0), "left": (1.0, 10.0, 0.0)}
WAIST = (0.0, 12.0, 0.0)
# Bent limbs (mod/core/.../SkinLimbs.java): the joint from the part's pivot, and the far end from
# the joint (the hand's or foot's point), in the part's own space.
UPPER = {"rightArm": (-1.0, 4.0, 0.0), "leftArm": (1.0, 4.0, 0.0), "rightLeg": (0.0, 6.0, 0.0), "leftLeg": (0.0, 6.0, 0.0)}
LOWER = (0.0, 6.0, 0.0)
JOINT = {"rightArm": "rightForearm", "leftArm": "leftForearm", "rightLeg": "rightShin", "leftLeg": "leftShin"}
PIVOT = {"rightArm": (-5.0, 2.0, 0.0), "leftArm": (5.0, 2.0, 0.0), "rightLeg": (-1.9, 12.0, 0.0), "leftLeg": (1.9, 12.0, 0.0)}
HIPS = {"right": (-1.9, 12.0, 0.0), "left": (1.9, 12.0, 0.0)}
GROUND = 24.0


# ---- curves ---------------------------------------------------------------------------------

def ease(k):
    return k * k * (3 - 2 * k)


def sample(keys, t):
    """A study track ([[time, value], ...], value a number or [x, y, z]) at t seconds."""
    if t <= keys[0][0]:
        return keys[0][1]
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t <= t1:
            k = ease((t - t0) / (t1 - t0)) if t1 > t0 else 1.0
            if isinstance(v0, list):
                return [a + (b - a) * k for a, b in zip(v0, v1)]
            return v0 + (v1 - v0) * k
    return keys[-1][1]


def times(length):
    n = int(round(length * FPS))
    return [min(i / FPS, length) for i in range(n + 1)]


def thin(frames):
    """Drop keys that a straight line between the kept neighbours already gives."""
    if len(frames) <= 2:
        return frames
    kept = [frames[0]]
    for i in range(1, len(frames) - 1):
        (ta, va), (tb, vb), (tc, vc) = kept[-1], frames[i], frames[i + 1]
        k = (tb - ta) / (tc - ta)
        if any(abs(a + (c - a) * k - b) > TOLERANCE for a, b, c in zip(va, vb, vc)):
            kept.append(frames[i])
    kept.append(frames[-1])
    return kept


# ---- rotations (the model part's order: Rz * Ry * Rx) ------------------------------------------

def euler(x, y, z):
    cx, sx, cy, sy, cz, sz = math.cos(x), math.sin(x), math.cos(y), math.sin(y), math.cos(z), math.sin(z)
    return [[cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
            [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
            [-sy, cy * sx, cy * cx]]


def deg_euler(v):
    return euler(*(math.radians(a) for a in v))


def apply(m, v):
    return [sum(m[r][c] * v[c] for c in range(3)) for r in range(3)]


def transpose(m):
    return [[m[c][r] for c in range(3)] for r in range(3)]


def matmul(a, b):
    return [[sum(a[r][k] * b[k][c] for k in range(3)) for c in range(3)] for r in range(3)]


def turn_between(a, b):
    """The smallest rotation taking unit vector a onto unit vector b (Rodrigues)."""
    axis = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    s = math.sqrt(sum(c * c for c in axis))
    c = max(-1.0, min(1.0, sum(x * y for x, y in zip(a, b))))
    if s < 1e-9:
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    k = [v / s for v in axis]
    angle = math.atan2(s, c)
    sin, cos = math.sin(angle), math.cos(angle)
    kx, ky, kz = k
    return [[cos + kx * kx * (1 - cos), kx * ky * (1 - cos) - kz * sin, kx * kz * (1 - cos) + ky * sin],
            [ky * kx * (1 - cos) + kz * sin, cos + ky * ky * (1 - cos), ky * kz * (1 - cos) - kx * sin],
            [kz * kx * (1 - cos) - ky * sin, kz * ky * (1 - cos) + kx * sin, cos + kz * kz * (1 - cos)]]


def angles(m):
    """x, y, z (degrees) of a rotation made as euler()."""
    y = math.asin(max(-1.0, min(1.0, -m[2][0])))
    return [math.degrees(math.atan2(m[2][1], m[2][2])), math.degrees(y), math.degrees(math.atan2(m[1][0], m[0][0]))]


def rotation_about(axis, angle):
    """Rodrigues: turning by angle (radians) about a unit axis."""
    kx, ky, kz = axis
    sin, cos = math.sin(angle), math.cos(angle)
    return [[cos + kx * kx * (1 - cos), kx * ky * (1 - cos) - kz * sin, kx * kz * (1 - cos) + ky * sin],
            [ky * kx * (1 - cos) + kz * sin, cos + ky * ky * (1 - cos), ky * kz * (1 - cos) - kx * sin],
            [kz * kx * (1 - cos) - ky * sin, kz * ky * (1 - cos) + kx * sin, cos + kz * kz * (1 - cos)]]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


class Follow:
    """A segment that points a new way each frame.

    It turns by the smallest rotation from where it points now (so it never flips between
    equivalent poses), and its twist about its own length eases back toward the plain swing from
    rest, which turning step by step would otherwise let drift (a limb back at rest must not stay
    twisted). Near pointing straight opposite its rest, where that swing is undefined, the twist
    is left alone.
    """

    # Share of the twist error removed per frame.
    UNTWIST = 0.25
    # Pointing closer than this (cosine) to straight opposite its rest: no untwisting.
    OPPOSITE = -0.8

    def __init__(self, local):
        length = math.sqrt(dot(local, local))
        self.rest = [c / length for c in local]
        side = [1.0, 0.0, 0.0] if abs(self.rest[0]) < 0.9 else [0.0, 0.0, 1.0]
        side = cross(self.rest, side)
        n = math.sqrt(dot(side, side))
        self.side = [c / n for c in side]
        self.rotation = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        self.previous = [0.0, 0.0, 0.0]

    def towards(self, want):
        norm = math.sqrt(dot(want, want)) or 1.0
        want = [c / norm for c in want]
        now = apply(self.rotation, self.rest)
        self.rotation = matmul(turn_between(now, want), self.rotation)
        if dot(want, self.rest) > self.OPPOSITE:
            swing = turn_between(self.rest, want)
            have, plain = apply(self.rotation, self.side), apply(swing, self.side)
            twist = math.atan2(dot(cross(have, plain), want), dot(have, plain))
            self.rotation = matmul(rotation_about(want, twist * self.UNTWIST), self.rotation)
        self.previous = [p + math.remainder(a - p, 360.0) for a, p in zip(angles(self.rotation), self.previous)]
        return self.previous


class Arm:
    """A straight arm whose hand follows a target (see Follow)."""

    def __init__(self, side):
        self.side = side
        self.follow = Follow(HAND[side])

    def towards(self, target_bb, torso_deg):
        target = (target_bb[0], GROUND - target_bb[1], target_bb[2])
        torso = deg_euler(torso_deg)
        shoulder = [w + d for w, d in zip(WAIST, apply(torso, [s - w for s, w in zip(SHOULDER[self.side], WAIST)]))]
        return self.follow.towards(apply(transpose(torso), [a - b for a, b in zip(target, shoulder)]))


def joint_between(start, end, upper, lower, pole):
    """Where the joint of a two-part limb goes so it reaches from start to end, bent toward pole."""
    d = [e - s for s, e in zip(start, end)]
    dist = math.sqrt(sum(c * c for c in d)) or 1e-6
    direction = [c / dist for c in d]
    dist = max(abs(upper - lower) + 1e-3, min(upper + lower - 1e-3, dist))
    along = (upper * upper - lower * lower + dist * dist) / (2 * dist)
    out = math.sqrt(max(upper * upper - along * along, 0.0))
    p = [q - s for s, q in zip(start, pole)]
    p = [a - sum(x * y for x, y in zip(p, direction)) * b for a, b in zip(p, direction)]
    norm = math.sqrt(sum(c * c for c in p))
    p = [c / norm for c in p] if norm > 1e-6 else [0.0, 0.0, -1.0]
    return [s + direction[i] * along + p[i] * out for i, s in enumerate(start)]


class BentLimb:
    """A limb bent at the elbow or knee whose end follows a target, bending toward a hint."""

    def __init__(self, part):
        self.part = part
        self.upper = Follow(UPPER[part])
        self.lower = Follow(LOWER)

    def towards(self, target_bb, hint_bb, torso_deg):
        """(upper, lower) angles in degrees; targets in Blockbench space, arms on the torso."""
        target = (target_bb[0], GROUND - target_bb[1], target_bb[2])
        hint = (hint_bb[0], GROUND - hint_bb[1], hint_bb[2])
        on_torso = self.part.endswith("Arm")
        torso = deg_euler(torso_deg) if on_torso else deg_euler([0.0, 0.0, 0.0])
        pivot = PIVOT[self.part]
        start = [w + d for w, d in zip(WAIST, apply(torso, [s - w for s, w in zip(pivot, WAIST)]))] if on_torso else list(pivot)
        return self.solve(target, hint, torso, start)

    def solve(self, target, hint, frame, start):
        """(upper, lower) angles in degrees for game-space target and hint, the limb's parent turned
        by frame and its pivot at start."""
        back = transpose(frame)
        local_target = apply(back, [a - b for a, b in zip(target, start)])
        local_hint = apply(back, [a - b for a, b in zip(hint, start)])
        upper_len = math.sqrt(sum(c * c for c in UPPER[self.part]))
        lower_len = math.sqrt(sum(c * c for c in LOWER))
        joint = joint_between([0.0, 0.0, 0.0], local_target, upper_len, lower_len, local_hint)
        upper = self.upper.towards(joint)
        reached = apply(self.upper.rotation, list(UPPER[self.part]))
        in_upper = apply(transpose(self.upper.rotation), [a - b for a, b in zip(local_target, reached)])
        lower = self.lower.towards(in_upper)
        return upper, lower


# A split body (chest and pelvis) turns about its middle, where the halves meet.
MID = (0.0, 6.0, 0.0)
# Where planted feet stand, game space (the middle of each foot's sole), and where knees point.
FOOT = {"rightLeg": (-1.9, GROUND, 0.0), "leftLeg": (1.9, GROUND, 0.0)}
KNEE_HINT = {"rightLeg": (-3.0, 18.0, -6.0), "leftLeg": (3.0, 18.0, -6.0)}


def split_body(spec, tracks, length):
    """A study with a separate pelvis (squat and sway move the body, feet stay planted): bones root,
    chest (the study's torso), pelvis (turned relative to the chest), head, arms, legs and shins."""
    zero = [[0.0, [0.0, 0.0, 0.0]]]
    num = lambda name: tracks.get(name, [[0.0, 0.0]])
    legs = {part: BentLimb(part) for part in ("rightLeg", "leftLeg")}
    channels = {k: [] for k in ("root.rotation", "root.position", "chest", "pelvis", "head", "rightArm", "leftArm",
                                "rightLeg", "leftLeg", "rightShin", "leftShin")}
    for t in times(length):
        chest = sample(tracks.get("torso", zero), t)
        pelvis = deg_euler(sample(tracks.get("pelvis", zero), t))
        relative = angles(matmul(transpose(deg_euler(chest)), pelvis))
        squat = sample(num("squat"), t)
        sway = sample(num("sway"), t) + sample(num("shift"), t)
        channels["root.rotation"].append((t, [0.0, sample(num("turn"), t), 0.0]))
        channels["root.position"].append((t, [sway, sample(num("lift"), t) - squat, 0.0]))
        channels["chest"].append((t, chest))
        channels["pelvis"].append((t, relative))
        channels["head"].append((t, sample(tracks.get("head", zero), t)))
        channels["rightArm"].append((t, sample(tracks.get("rightArm", zero), t)))
        channels["leftArm"].append((t, sample(tracks.get("leftArm", zero), t)))
        for part, solver in legs.items():
            # The root moves the whole player by (sway, -squat): planted feet stay where they were.
            foot = (FOOT[part][0] - sway, FOOT[part][1] - squat, FOOT[part][2])
            hint = (KNEE_HINT[part][0] - sway, KNEE_HINT[part][1] - squat, KNEE_HINT[part][2])
            hip = [m + d for m, d in zip(MID, apply(pelvis, [p - m for p, m in zip(PIVOT[part], MID)]))]
            upper, lower = solver.solve(foot, hint, pelvis, hip)
            channels[part].append((t, upper))
            channels[JOINT[part]].append((t, lower))
    return channels


def lowest_foot(legs_deg):
    """How far the lowest foot corner sits above the ground (negative: below), in pixels."""
    deepest = -1e9
    for side, rot in legs_deg.items():
        m = deg_euler(rot)
        for cx in (-2.0, 2.0):
            for cz in (-2.0, 2.0):
                p = apply(m, [cx, 12.0, cz])
                deepest = max(deepest, HIPS[side][1] + p[1])
    return GROUND - deepest


# ---- one study --------------------------------------------------------------------------------

def convert(spec):
    tracks = spec["tracks"]
    length = float(spec["durationSeconds"])
    zero = [[0.0, [0.0, 0.0, 0.0]]]

    def vec(name):
        return tracks.get(name, zero)

    def num(name):
        return tracks.get(name, [[0.0, 0.0]])

    if "pelvis" in tracks:
        return animation_file(spec, length, split_body(spec, tracks, length))
    right_target = tracks.get("rightHand") or tracks.get("hand")
    left_target = tracks.get("leftHand")
    arms = {"right": Arm("right"), "left": Arm("left")}
    # Bent limbs: (end target, bend hint) tracks per part.
    bent = {}
    if "elbowHint" in tracks and right_target:
        bent["rightArm"] = (right_target, tracks["elbowHint"])
    for side in ("right", "left"):
        if f"{side}Foot" in tracks and f"{side}KneeHint" in tracks:
            bent[f"{side}Leg"] = (tracks[f"{side}Foot"], tracks[f"{side}KneeHint"])
    solvers = {part: BentLimb(part) for part in bent}
    channels = {k: [] for k in ("root.rotation", "root.position", "torso", "head", "rightArm", "leftArm", "rightLeg", "leftLeg")}
    for part in bent:
        channels[JOINT[part]] = []
    for t in times(length):
        torso = sample(vec("torso"), t)
        legs = {"right": sample(vec("rightLeg"), t), "left": sample(vec("leftLeg"), t)}
        right = arms["right"].towards(sample(right_target, t), torso) if right_target else sample(vec("rightArm"), t)
        left = arms["left"].towards(sample(left_target, t), torso) if left_target else sample(vec("leftArm"), t)
        if "bounce" in tracks:
            lift = -lowest_foot(legs) + sample(num("bounce"), t)
        else:
            lift = sample(num("lift"), t)
        channels["root.rotation"].append((t, [0.0, sample(num("turn"), t), 0.0]))
        channels["root.position"].append((t, [sample(num("shift"), t), lift, 0.0]))
        channels["torso"].append((t, torso))
        channels["head"].append((t, sample(vec("head"), t)))
        channels["rightArm"].append((t, right))
        channels["leftArm"].append((t, left))
        channels["rightLeg"].append((t, legs["right"]))
        channels["leftLeg"].append((t, legs["left"]))
        for part, (target, hint) in bent.items():
            upper, lower = solvers[part].towards(sample(target, t), sample(hint, t), torso)
            channels[part][-1] = (t, upper)
            channels[JOINT[part]].append((t, lower))
    return animation_file(spec, length, channels)


def animation_file(spec, length, channels):
    bones = {}
    for key, frames in channels.items():
        bone, kind = key.split(".") if "." in key else (key, "rotation")
        kept = thin(frames)
        if all(abs(c) < 1e-6 for _, v in kept for c in v) and bone != "root":
            continue
        bones.setdefault(bone, {})[kind] = {f"{t:.4f}".rstrip("0").rstrip("."): [round(c, 3) for c in v] for t, v in kept}
    # The rig is chosen by the root bone: always written, even when it never moves.
    bones.setdefault("root", {}).setdefault("position", {"0": [0, 0, 0]})
    return {"format_version": "1.8.0",
            "animations": {f"animation.{spec['id'].replace('-', '_')}": {
                "loop": False, "animation_length": round(length, 3), "bones": bones}}}


def main():
    check = "--check" in sys.argv
    catalog_path = os.path.join(ASSETS, "emotes.json")
    with open(catalog_path, encoding="utf-8") as f:
        catalog = json.load(f)
    known = {e["id"] for e in catalog}
    skipped = []
    for path in sorted(glob.glob(os.path.join(SPECS, "**", "motion-spec.json"), recursive=True)):
        folder = os.path.basename(os.path.dirname(path))
        with open(path, encoding="utf-8") as f:
            spec = json.load(f)
        joints = NEEDS_JOINTS & set(spec["tracks"])
        if folder not in EMOTES or joints:
            skipped.append(f"{folder} ({', '.join(sorted(joints)) or 'not listed'})")
            continue
        emote_id, name = EMOTES[folder]
        animation = convert(spec)
        out = os.path.join(ASSETS, "emotes", f"{emote_id}.animation.json")
        keys = sum(len(ch) for b in animation["animations"].popitem()[1]["bones"].values() for ch in b.values()) \
            if False else None
        print(f"{emote_id}: {spec['durationSeconds']} s")
        if not check:
            with open(out, "w", encoding="utf-8", newline="\n") as f:
                json.dump(animation, f, separators=(",", ":"))
                f.write("\n")
        if emote_id not in known:
            catalog.append({"id": emote_id, "name": name})
            known.add(emote_id)
    if not check:
        with open(catalog_path, "w", encoding="utf-8", newline="\n") as f:
            f.write("[\n" + ",\n".join("  " + json.dumps(e) for e in catalog) + "\n]\n")
    for s in skipped:
        print(f"skipped {s}")


if __name__ == "__main__":
    main()
