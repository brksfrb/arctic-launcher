"""Prepare only a fresh isolated test world's visual conditions before entry."""
import hashlib
import json
from pathlib import Path
import struct

SETTINGS = {"Difficulty": (3, 0), "ForceGameType": (1, 1), "GameType": (3, 1),
            "domobspawning": (1, 0), "spawnMobs": (1, 0)}
LAN_SETTINGS = {"MultiplayerGame": (1, 1), "MultiplayerGameIntent": (1, 1),
                "LANBroadcast": (1, 1), "LANBroadcastIntent": (1, 1),
                "XBLBroadcastIntent": (3, 0), "PlatformBroadcastIntent": (3, 0)}


def prepare_bytes(original, lan=False):
    if (len(original) < 12 or len(original) > 16 * 1024 * 1024
            or struct.unpack_from("<I", original)[0] != 10
            or struct.unpack_from("<I", original, 4)[0] != len(original) - 8):
        raise ValueError("unsupported or truncated isolated level.dat")
    data = bytearray(original)
    position = 8
    visited = 0

    def take(count):
        nonlocal position
        if count < 0 or position + count > len(data):
            raise ValueError("truncated level NBT")
        start = position
        position += count
        return start

    def number(fmt):
        return struct.unpack_from(fmt, data, take(struct.calcsize(fmt)))[0]

    def text():
        length = number("<H")
        start = take(length)
        return data[start:start + length].decode("utf-8")

    def payload(kind, depth):
        nonlocal visited
        visited += 1
        if depth > 32 or visited > 1_000_000:
            raise ValueError("level NBT exceeds traversal limit")
        sizes = {1: 1, 2: 2, 3: 4, 4: 8, 5: 4, 6: 8}
        if kind in sizes:
            take(sizes[kind])
        elif kind == 8:
            text()
        elif kind in (7, 11, 12):
            count = number("<i")
            if not 0 <= count <= 1_000_000:
                raise ValueError("invalid NBT array size")
            take(count * {7: 1, 11: 4, 12: 8}[kind])
        elif kind == 9:
            child, count = number("<B"), number("<i")
            if not 0 <= count <= 1_000_000 or child == 0 and count:
                raise ValueError("invalid NBT list")
            for _ in range(count):
                payload(child, depth + 1)
        elif kind == 10:
            while (child := number("<B")) != 0:
                text()
                payload(child, depth + 1)
        else:
            raise ValueError("unknown NBT tag")

    if number("<B") != 10:
        raise ValueError("level NBT root is not a compound")
    text()
    fields = {}
    while (kind := number("<B")) != 0:
        name = text()
        if name in fields:
            raise ValueError("duplicate level NBT field")
        start = position
        payload(kind, 1)
        fields[name] = (kind, start)
    if position != len(data):
        raise ValueError("trailing level NBT data")
    changed = {}
    for name, (kind, value) in (SETTINGS | (LAN_SETTINGS if lan else {})).items():
        if name not in fields or fields[name][0] != kind:
            raise ValueError(f"missing or incompatible test-world setting: {name}")
        at = fields[name][1]
        fmt = "<i" if kind == 3 else "<b"
        old = struct.unpack_from(fmt, data, at)[0]
        struct.pack_into(fmt, data, at, value)
        changed[name] = {"before": old, "after": value}
    return bytes(data), changed


def prepare(root, lan=False):
    root = Path(root).resolve(strict=True)
    config = json.loads((root / "arctic-offline-probe.json").read_text(encoding="utf-8-sig"))
    profile = (root / "game-profile").resolve(strict=True)
    live = json.loads((root / "probe-live.json").read_text())
    engine = json.loads((root / f"arctic-bedrock-{live['pid']}.engine.json").read_text())
    if (config.get("schema") != 1 or config.get("auto_world") is not True
            or config.get("empty_user_provider") is not True
            or config.get("mode") != "loopback-and-deny-user-provider"
            or Path(config.get("profile", "")).resolve() != profile
            or live.get("privateDesktop") is not True or live.get("isolatedBoot") is not True
            or engine.get("worldPasses") != 0):
        raise ValueError("world preparation requires an isolated game before world entry")
    if lan and config.get("lan_record_probe") is not True:
        raise ValueError("LAN preparation requires an explicit isolated LAN recording probe")
    files = sorted(profile.glob("**/minecraftWorlds/*/level.dat"))
    if not 1 <= len(files) <= 8:
        raise ValueError("unexpected isolated world count")
    pending = []
    for file in files:
        resolved = file.resolve(strict=True)
        if not resolved.is_relative_to(profile) or resolved != file:
            raise ValueError("isolated level.dat escaped the test profile")
        original = file.read_bytes()
        prepared, changed = prepare_bytes(original, lan)
        pending.append((file, original, prepared, changed))
    evidence = []
    for file, original, prepared, changed in pending:
        if file.read_bytes() != original:
            raise ValueError("level.dat changed before test preparation")
        temporary = file.with_name("level.dat.arctic-preparing")
        temporary.write_bytes(prepared)
        temporary.replace(file)
        evidence.append({"file": str(file.relative_to(profile)), "settings": changed,
                         "beforeSha256": hashlib.sha256(original).hexdigest(),
                         "afterSha256": hashlib.sha256(prepared).hexdigest()})
    report = {"schema": 1, "preparedBeforeWorldEntry": True, "lanRecordProbe": lan, "worlds": evidence}
    (root / "world-setup.json").write_text(json.dumps(report), encoding="utf-8")
    return report
