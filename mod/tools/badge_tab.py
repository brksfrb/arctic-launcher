"""The Arctic logo (style 0) for the player list: a square 9 GUI px picture, the same height as before.

The list draws the picture with nearest-neighbour sampling, so a texture with many more texels than screen
pixels drops thin lines at some GUI scales: the snowflake's up/down arms vanished first and the logo read
wide. Here the art is area-averaged onto an 18 x 18 grid (a texel is half a GUI px, so one screen pixel at GUI
scale 2) and each texel is a flat 4 x 4 block of the 72 x 72 file the fonts and the old versions' drawing code expect:
every sample lands inside one block, thin arms stay as partly-covered texels at every scale, and both axes
are sampled alike. The art keeps its own proportions, centred in the square, with one texel of alpha 1/255 in
the last column so the glyph is 9 px wide as well as 9 px tall (a bitmap glyph is as wide as its last
non-empty column).
"""
from pathlib import Path
from PIL import Image

GRID = 18
BLOCK = 4
FILL = 17.0


def tab_art(logo):
    """The 72 x 72 tab picture from the logo at any size (RGBA, transparent around the mark)."""
    box = logo.getchannel('A').point(lambda a: 255 if a > 8 else 0).getbbox()
    mark = logo.crop(box)
    height = FILL
    width = height * mark.width / mark.height
    sub = 8
    canvas = Image.new('RGBA', (GRID * sub, GRID * sub), (0, 0, 0, 0))
    scaled = mark.convert('RGBa').resize((round(width * sub), round(height * sub)), Image.Resampling.LANCZOS).convert('RGBA')
    canvas.alpha_composite(scaled, (round((GRID - width) / 2 * sub), round((GRID - height) / 2 * sub)))
    grid = canvas.convert('RGBa').resize((GRID, GRID), Image.Resampling.BOX).convert('RGBA')
    # Thin arms cover part of a texel: lift that coverage so they read as lines, not as haze.
    grid.putalpha(grid.getchannel('A').point(lambda a: round(255 * (a / 255) ** 0.6)))
    px = grid.load()
    # Widen the glyph to the full square: a nearly invisible texel in the last column, beside the nearest colour.
    r, g, b, _ = next(px[x, GRID // 2] for x in range(GRID - 1, -1, -1) if px[x, GRID // 2][3] > 0)
    if px[GRID - 1, GRID // 2][3] == 0:
        px[GRID - 1, GRID // 2] = (r, g, b, 1)
    return grid.resize((GRID * BLOCK, GRID * BLOCK), Image.Resampling.NEAREST)


def write(logo, assets):
    art = tab_art(logo)
    art.save(assets / 'textures/font/badge_tab.png')
    art.save(assets / 'badge_tab.png')


if __name__ == '__main__':
    assets = Path(__file__).resolve().parents[1] / 'core/src/main/resources/assets/arctic'
    write(Image.open(assets / 'badge.png').convert('RGBA'), assets)
