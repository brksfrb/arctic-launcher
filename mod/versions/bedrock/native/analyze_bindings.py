"""Check candidate hook signatures against a package-context image read.

Signature patterns adapted from LatiteClient/Latite src/mc/Addresses.h at
9f7463515dd298a496da918285936d78c7416aad (GPL-3.0). See THIRD_PARTY.md.
No game code is executed or patched. A unique byte match is not ABI validation.
"""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

PATTERNS = {
    "platformGlobal": ("4C 89 3D ? ? ? ? 4D 85 FF", 3, 7),
    "renderWorld": ("E8 ? ? ? ? 45 31 E4 48 83 BE", 1, 5),
    "renderActor": ("55 41 57 41 56 41 55 41 54 56 57 53 48 81 EC ? ? ? ? 48 8D AC 24 ? ? ? ? 0F 29 7D ? 0F 29 75 ? 48 C7 45 ? ? ? ? ? 4C 89 C6 48 89 D7 48 89 CB", None, None),
    "levelTick": ("55 56 57 53 48 81 EC ? ? ? ? 48 8D AC 24 ? ? ? ? 44 0F 29 6D ? 44 0F 29 65 ? 44 0F 29 5D ? 44 0F 29 55 ? 66 44 0F 7F 4D", None, None),
    "gameUpdate": ("E8 ? ? ? ? 48 8B 8F ? ? ? ? BA ? ? ? ? E8 ? ? ? ? 48 8B 9F", 1, 5),
}


def relocation_facts(report, metadata, text):
    """Establish whether ASLR changes bytes covered by the executable hash."""
    if "relocationSize" not in metadata:
        return {}
    size, rva = metadata["relocationSize"], metadata["relocationRva"]
    section = next(s for s in metadata["sections"] if s["rva"] <= rva < s["rva"] + s["size"])
    data = report.with_suffix(f'.{section["rva"]:x}.bin').read_bytes()
    offset, end, count, code = rva - section["rva"], rva - section["rva"] + size, 0, 0
    if end > len(data):
        raise ValueError("truncated relocation directory")
    while offset < end:
        if offset + 8 > end:
            raise ValueError("truncated relocation block")
        page, length = struct.unpack_from("<II", data, offset)
        if length < 8 or length % 2 or offset + length > end:
            raise ValueError("invalid relocation block")
        for entry, in struct.iter_unpack("<H", data[offset + 8:offset + length]):
            kind, target = entry >> 12, page + (entry & 4095)
            if kind:
                count += 1
                if text["rva"] <= target < text["rva"] + text["size"]:
                    code += 1
        offset += length
    return {"preferredBase": metadata["preferredBase"], "relocations": count, "textRelocations": code}


def analyze(report):
    metadata = json.loads(report.read_text())
    if not metadata.get("readable") or metadata.get("gameStarted") is not False:
        raise ValueError("expected a successful read-only package inspection")
    text = next(s for s in metadata["sections"] if s["name"] == ".text")
    data = report.with_suffix(f'.{text["rva"]:x}.bin').read_bytes()
    if len(data) != text["size"]:
        raise ValueError("executable section is truncated")
    matches = {}
    for name, (pattern, displacement, end) in PATTERNS.items():
        expression = b"".join(b"." if part == "?" else re.escape(bytes([int(part, 16)])) for part in pattern.split())
        hits = [match.start() for match in re.finditer(expression, data, re.DOTALL)]
        record = {"matches": len(hits), "unique": len(hits) == 1}
        if len(hits) == 1:
            record["matchRva"] = hits[0] + text["rva"]
            record["targetRva"] = record["matchRva"] if displacement is None else record["matchRva"] + end + struct.unpack_from("<i", data, hits[0] + displacement)[0]
            if not any(s["rva"] <= record["targetRva"] < s["rva"] + max(s["size"], s.get("virtualSize", 0)) for s in metadata["sections"]):
                raise ValueError("signature resolved outside the inspected image")
        matches[name] = record
    return {"schema": 1, "installedFileSize": metadata["size"], "textSha256": hashlib.sha256(data).hexdigest(),
            **relocation_facts(report, metadata, text),
            "machine": metadata.get("machine"), "timestamp": metadata.get("timestamp"), "imageSize": metadata.get("imageSize"),
            "candidates": matches, "gameStarted": False, "liveCosmeticsVerified": False,
            "remaining": ["Validate actor identity/pose layouts", "Bind matching world camera and terrain depth", "Verify renderer replacement in Minecraft"]}


if __name__ == "__main__":
    report = Path(sys.argv[1]).resolve()
    result = analyze(report)
    output = report.with_name("binding-facts.json")
    with output.open("x", encoding="utf-8") as file:
        json.dump(result, file, indent=2)
        file.write("\n")
    print(json.dumps(result, indent=2))
