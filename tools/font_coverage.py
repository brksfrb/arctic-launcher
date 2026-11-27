"""Report outline glyph coverage for launcher locales and Rust string literals.

Run with Python and FontTools. Pass fonts in egui fallback order, with a :FACE
suffix for a TrueType collection. --codepoints prints a pyftsubset Unicode list.
"""
import argparse
import json
import re
import sys
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path

from i18n_check import ROOT, literals, strip_comments

APP = ROOT / "crates" / "arctic-app"
RUST_UNICODE = re.compile(r"\\u\{([0-9a-fA-F_]+)\}")


def required():
    groups = defaultdict(set)
    for path in sorted((APP / "locales").glob("*/*.json")):
        for value in json.loads(path.read_text(encoding="utf-8")).values():
            groups[path.parent.name].update(value)
    for path in sorted((APP / "src").rglob("*.rs")):
        source = strip_comments(path.read_text(encoding="utf-8"))
        for _, value in literals(source, java=False):
            groups["Rust UI"].update(value)
            groups["Rust UI"].update(
                chr(int(match, 16)) for match in RUST_UNICODE.findall(value)
            )
    return {
        group: {
            c for c in chars
            if (c.isprintable() or unicodedata.category(c) == "Zs")
            and not 0xFE00 <= ord(c) <= 0xFE0F
        }
        for group, chars in groups.items()
    }


def outlines(spec):
    from fontTools.pens.boundsPen import BoundsPen
    from fontTools.ttLib import TTFont

    path, separator, face = spec.rpartition(":")
    with TTFont(path if separator and face.isdigit() else spec,
                fontNumber=int(face) if separator and face.isdigit() else 0) as font:
        family = font["name"].getBestFamilyName()
        if not any(table in font for table in ("glyf", "CFF ", "CFF2")):
            return family, set()
        glyphs = font.getGlyphSet()
        cmap = font.getBestCmap() or {}
        covered = set()
        for c in ALL:
            name = cmap.get(ord(c))
            if name not in glyphs:
                continue
            if c.isspace():
                covered.add(c)
                continue
            pen = BoundsPen(glyphs)
            glyphs[name].draw(pen)
            if pen.bounds:
                covered.add(c)
    return family, covered


GROUPS = required()
# Test the whole printable ASCII range and the replacement character too.
ALL = set().union(*GROUPS.values(), map(chr, range(32, 127)), "�✓⚙▶")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fonts", nargs="*")
    parser.add_argument("--codepoints", action="store_true")
    args = parser.parse_args()
    if args.codepoints:
        print(",".join(f"U+{ord(c):04X}" for c in sorted(ALL)))
        return 0
    if not args.fonts:
        parser.error("supply at least one font or use --codepoints")

    available = {}
    for spec in args.fonts:
        name, covered = outlines(spec)
        for c in covered:
            available.setdefault(c, name)

    for group, chars in sorted(GROUPS.items()):
        non_ascii = {c for c in chars if not c.isascii()}
        counts = Counter(available.get(c, "MISSING") for c in non_ascii)
        families = ", ".join(f"{name} ({count})" for name, count in sorted(counts.items()))
        print(f"{group:7} {len(non_ascii):4} non-ASCII glyphs   {families}")
    missing = ALL - available.keys()
    if missing:
        print("Missing glyphs")
        for c in sorted(missing):
            print(f"  U+{ord(c):04X} {c} {unicodedata.name(c, 'UNNAMED')}")
    return bool(missing)


if __name__ == "__main__":
    sys.exit(main())
