"""Render the Arctic Client's mod icons: white line icons, 64x64 PNGs.

Usage: python mod/tools/icons.py [--preview out.png]
Writes mod/core/src/main/resources/assets/arctic/icons/<name>.png.

Most icons are Lucide icons (https://lucide.dev, ISC license, see
icon-svg/LICENSE-lucide.txt), kept in icon-svg/lucide/ so this runs offline;
a missing one is downloaded once. The few Lucide has nothing for are drawn
here as SVG in the same style (24-unit grid, 2-unit round strokes).
Rendering uses resvg (pip install resvg-py), so edges come out clean.
"""
import sys
import urllib.request
from pathlib import Path

import resvg_py

ROOT = Path(__file__).resolve().parent
OUT = ROOT.parent / "core/src/main/resources/assets/arctic/icons"
LUCIDE = ROOT / "icon-svg/lucide"
LUCIDE_URL = "https://unpkg.com/lucide-static@1.48.0/icons/{}.svg"
SIZE = 64

# Icon name (what the mod asks for) -> Lucide icon.
FROM_LUCIDE = {
    "account": "user",
    "armor": "shield",
    "arrows": "bow-arrow",
    "autogg": "trophy",
    "back": "chevron-left",
    "biome": "mountain-snow",
    "camera": "camera",
    "chat": "message-square",
    "chunk": "grid-3x3",
    "clock": "clock",
    "combo": "swords",
    "compass": "compass",
    "coords": "locate-fixed",
    "cps": "mouse-pointer-click",
    "crosshair": "crosshair",
    "date": "calendar-days",
    "day": "sunrise",
    "death": "skull",
    "direction": "navigation-2",
    "durability": "pickaxe",
    "effects": "flask-conical",
    "entities": "rabbit",
    "food": "drumstick",
    "fps": "gauge",
    "freelook": "eye",
    "friends": "users",
    "fullbright": "sun",
    "held": "sword",
    "hitcolor": "heart-crack",
    "hud": "layout-dashboard",
    "items": "package",
    "keystrokes": "keyboard",
    "leave": "log-out",
    "light": "lightbulb",
    "looks": "shirt",
    "memory": "memory-stick",
    "messages": "message-square-text",
    "minimap": "map",
    "motionblur": "fast-forward",
    "itemphysics": "package-open",
    "mods": "layout-grid",
    "movement": "footprints",
    "outline": "box",
    "pack": "image",
    "packs": "layers",
    "ping": "signal",
    "players": "users-round",
    "reach": "ruler",
    "replay": "history",
    "scoreboard": "clipboard-list",
    "search": "search",
    "server": "server",
    "session": "hourglass",
    "settings": "settings",
    "speed": "wind",
    "stopwatch": "timer",
    "streamer": "eye-off",
    "style": "palette",
    "target": "target",
    "time": "sun-moon",
    "tps": "activity",
    "voice": "mic",
    "waypoints": "map-pin",
    "weather": "cloud-off",
    "worldtime": "sunset",
    "zoom": "zoom-in",
}

# Drawn here: the SVG elements inside a 24x24 Lucide-style frame.
CUSTOM = {
    # Running figure.
    "sprint": """
        <circle cx="16" cy="4.5" r="2"/>
        <path d="M11.5 20.5l2.5-5-3-3 2-5"/>
        <path d="M13 7.5l-4.5 1-2 3.5"/>
        <path d="M13 7.5l2 4 4 1.5"/>
        <path d="M11 12.5l-3.5 3.5-4 .5"/>
    """,
    # Crouching figure: head low, back bent forward, knees bent.
    "sneak": """
        <circle cx="14" cy="6.5" r="2"/>
        <path d="M12 9.5l-3.5 4.5"/>
        <path d="M8.5 14l5 1.5-2 5"/>
        <path d="M8.5 14l-2 3.5 1.5 3.5"/>
        <path d="M11 11l4 2 1.5 3"/>
    """,
    # A lower flame with a down arrow.
    "fire": """
        <g transform="translate(-1.5 4.5) scale(0.8)">
          <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"/>
        </g>
        <path d="M19 3v7"/>
        <path d="M16 7l3 3 3-3"/>
    """,
}

FRAME = ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" '
         'fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>')


def lucide(name):
    path = LUCIDE / f"{name}.svg"
    if not path.exists():
        LUCIDE.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(LUCIDE_URL.format(name), timeout=30) as r:
            path.write_bytes(r.read())
    return path.read_text(encoding="utf-8").replace("currentColor", "#ffffff")


def render(svg):
    return bytes(resvg_py.svg_to_bytes(svg_string=svg, width=SIZE, height=SIZE))


def sources():
    out = {name: lucide(icon) for name, icon in FROM_LUCIDE.items()}
    out.update({name: FRAME.format(body) for name, body in CUSTOM.items()})
    return out


def preview(pngs, path):
    """All icons on one dark sheet, for checking them by eye."""
    from io import BytesIO
    from PIL import Image, ImageDraw
    names = sorted(pngs)
    cols, cell = 10, 96
    sheet = Image.new("RGB", (cols * cell, ((len(names) + cols - 1) // cols) * cell), (22, 33, 52))
    d = ImageDraw.Draw(sheet)
    for i, name in enumerate(names):
        x, y = (i % cols) * cell, (i // cols) * cell
        icon = Image.open(BytesIO(pngs[name])).convert("RGBA")
        sheet.paste(icon, (x + 16, y + 6), icon)
        d.text((x + 6, y + 76), name, fill=(200, 210, 225))
    sheet.save(path)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    pngs = {name: render(svg) for name, svg in sources().items()}
    for name, png in pngs.items():
        (OUT / f"{name}.png").write_bytes(png)
    stale = {p.stem for p in OUT.glob("*.png")} - set(pngs)
    if stale:
        print("not drawn any more (still in the folder):", ", ".join(sorted(stale)))
    if "--preview" in sys.argv:
        preview(pngs, sys.argv[sys.argv.index("--preview") + 1])
    print(f"{len(pngs)} icons")


if __name__ == "__main__":
    main()
