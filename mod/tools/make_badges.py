"""Rasterize the three picked round-3 badges, using the official Arctic mark.

SVG sources are reviewable beside this script. Picker art keeps the sheet's
3-unit outline; tag/tab variants use 7 units (1.75/1.97 framebuffer px at GUI
scale 2). Filter at GUI scale 2 before expanding the atlas with nearest sampling, so
thin mark details cannot fall between the game's nearest-filtered texels.
"""
import io
import json
from pathlib import Path
import cairosvg
from PIL import Image, ImageFilter

MOD = Path(__file__).resolve().parents[1]
ASSETS = MOD / 'core/src/main/resources/assets'
ROOT = ASSETS / 'arctic'
STYLES = [('badge', 0), ('badge_ice_tile', 1), ('badge_line_tile', 2)]
# Recolor the approved SVG art; role variants are not additional selectable styles.
ROLES = [('', '#2EB1EC'), ('founder', '#FFC93C'), ('dev', '#3CE0FF'),
         ('admin', '#FF5A5A'), ('mod', '#5AE07A'), ('beta', '#B07CFF')]
providers = []
for base, style in STYLES:
    original = (MOD / 'tools/badges' / (base + '.svg')).read_text()
    for role_index, (role, color) in enumerate(ROLES):
        name = base + ('_' + role if role else '')
        rgb = tuple(int(color[i:i+2], 16) for i in (1, 3, 5))
        pale = '#' + ''.join(f'{round(c * .42 + 255 * .58):02X}' for c in rgb)
        svg = original.replace('#2EB1EC', color).replace('#A8DCF4', pale) if role else original
        def raster(source):
            return Image.open(io.BytesIO(cairosvg.svg2png(bytestring=source.encode(), output_width=1024, output_height=1024))).convert('RGBA')
        def glow(im, radius):
            if not role:
                return im
            alpha = im.getchannel('A').filter(ImageFilter.GaussianBlur(radius)).point(lambda a: round(a * .22))
            halo = Image.new('RGBA', im.size, rgb + (0,))
            halo.putalpha(alpha)
            return Image.alpha_composite(halo, im)
        im = raster(svg)
        glow(im.resize((256, 256), Image.Resampling.LANCZOS), 4).save(ROOT / (name + '.png'))
        small_art = raster(svg.replace('stroke-width="3"', 'stroke-width="7"')) if style == 2 else im
        for tab, height, ascent in [(False, 8, 7), (True, 9, 8)]:
            font = name + ('_tab' if tab else '')
            size = height * 8
            small = small_art.resize((height * 2, height * 2), Image.Resampling.LANCZOS)
            small = glow(small, .65).resize((size, size), Image.Resampling.NEAREST)
            small.save(ROOT / 'textures/font' / (font + '.png'))
            small.save(ROOT / (name + ('_tab' if tab else '_tag') + '.png'))
            bitmap = dict(type='bitmap', file='arctic:font/' + font + '.png', height=height, ascent=ascent, chars=['\uE000'])
            contents = [bitmap, dict(type='space', advances={' ': 1})]
            if name == 'badge' and not tab:
                contents.insert(1, dict(type='bitmap', file='arctic:font/speaker.png', height=8, ascent=7, chars=['\uE001']))
            (ROOT / 'font' / (font + '.json')).write_text(json.dumps(dict(providers=contents), indent=2) + '\n')
            glyph = 0xE0A0 + style + (0x10 if tab else 0) if not role else 0xE300 + (role_index - 1) * 6 + style + (3 if tab else 0)
            providers.append(dict(bitmap, chars=[chr(glyph)]))
providers.append(dict(type='space', advances={'\uE0AF': 1}))
(ASSETS / 'minecraft/font/default.json').write_text(json.dumps(dict(providers=providers), indent=2) + '\n')
for old in ['badge1', 'badge2', 'badge3', 'badge_shield', 'badge_shield_tab', 'badge_shield_tag']:
    for path in [ROOT / (old+'.png'), ROOT / 'font' / (old+'.json'), ROOT / 'textures/font' / (old+'.png')]:
        path.unlink(missing_ok=True)
