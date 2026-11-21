"""Generate the hand-drawn Arctic badge variants at their native GUI sizes."""
from pathlib import Path

from PIL import Image

ASSETS = Path(__file__).resolve().parents[1] / "core/src/main/resources/assets/arctic"

# Periods are transparent. Each other symbol is one fully opaque GUI pixel.
PALETTE = {
    "+": "#56C4F5",
    "*": "#CEEEFF",
    "i": "#A8DCF4",
    "d": "#146C9B",
    "l": "#299AD2",
}
DESIGNS = {
    ("badge", 8): (
        "...++...", "+..++..+", ".+.**.+.",
        "..+**+..", "..+**+..", ".+.**.+.", "+..++..+", "...++...",
    ),
    ("badge", 9): (
        "....+....", "....+....", "+.+.*.+.+", ".++***++.",
        "...***...", ".++***++.", "+.+.*.+.+", "....+....", "....+....",
    ),
    ("badge_ice_tile", 8): (
        "..iiii..", ".iidiii.", "idididii", "iiddddii",
        "iiddddii", "iidididi", ".iiidii.", "..iiii..",
    ),
    ("badge_ice_tile", 9): (
        "..iiiii..", ".iiidiii.", "idiidiidi", "iidddddii",
        "iiidddiii", "iidddddii", "idiidiidi", ".iiidiii.", "..iiiii..",
    ),
    ("badge_line_tile", 8): (
        "..llll..", ".l.++.l.", "l.+..+.l", "l..**..l",
        "l..**..l", "l.+..+.l", ".l.++.l.", "..llll..",
    ),
    ("badge_line_tile", 9): (
        "..lllll..", ".l..+..l.", "l.+.+.+.l", "l..***..l",
        "l.+***+.l", "l..***..l", "l.+.+.+.l", ".l..+..l.", "..lllll..",
    ),
}


def main():
    for (name, size), rows in DESIGNS.items():
        assert len(rows) == size and all(len(row) == size for row in rows)
        image = Image.new("RGBA", (size, size))
        for y, row in enumerate(rows):
            for x, pixel in enumerate(row):
                if pixel != ".":
                    color = PALETTE[pixel].removeprefix("#")
                    image.putpixel((x, y), (*bytes.fromhex(color), 255))
        placement = "tag" if size == 8 else "tab"
        image.save(ASSETS / f"{name}_{placement}_pixel.png")


if __name__ == "__main__":
    main()
