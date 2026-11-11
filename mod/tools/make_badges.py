"""Rasterize the three picked round-3 badges, using the official Arctic mark.

SVG sources are reviewable beside this script. Picker art keeps the sheet's
3-unit outline; tag/tab variants use 7 units (1.75/1.97 framebuffer px at GUI
scale 2). Font textures are rendered straight from the SVG at 8 texels per GUI pixel
(never prefiltered to a coarse grid and blown up), so the art keeps full alpha in its
body and stays sharp when a name tag is seen up close.
"""
import io
import json
from pathlib import Path
import cairosvg
from PIL import Image

MOD = Path(__file__).resolve().parents[1]
ASSETS = MOD / 'core/src/main/resources/assets'
ROOT = ASSETS / 'arctic'
STYLES = [('badge', 0), ('badge_ice_tile', 1), ('badge_line_tile', 2)]
providers = []
for name, style in STYLES:
    svg = (MOD / 'tools/badges' / (name + '.svg')).read_text()
    def raster(source):
        return Image.open(io.BytesIO(cairosvg.svg2png(bytestring=source.encode(), output_width=1024, output_height=1024))).convert('RGBA')
    im = raster(svg)
    im.resize((256, 256), Image.Resampling.LANCZOS).save(ROOT / (name + '.png'))
    small_art = raster(svg.replace('stroke-width="3"', 'stroke-width="7"')) if style == 2 else im
    for tab, height, ascent in [(False, 8, 7), (True, 9, 8)]:
        font = name + ('_tab' if tab else '')
        size = height * 8
        small = small_art.resize((size, size), Image.Resampling.LANCZOS)
        small.save(ROOT / 'textures/font' / (font + '.png'))
        small.save(ROOT / (name + ('_tab' if tab else '_tag') + '.png'))
        bitmap = dict(type='bitmap', file='arctic:font/' + font + '.png', height=height, ascent=ascent, chars=['\uE000'])
        contents = [bitmap, dict(type='space', advances={' ': 1})]
        if name == 'badge' and not tab:
            contents.insert(1, dict(type='bitmap', file='arctic:font/speaker.png', height=8, ascent=7, chars=['\uE001']))
        (ROOT / 'font' / (font + '.json')).write_text(json.dumps(dict(providers=contents), indent=2) + '\n')
        providers.append(dict(bitmap, chars=[chr(0xE0A0 + style + (0x10 if tab else 0))]))
providers.append(dict(type='space', advances={'\uE0AF': 1}))
(ASSETS / 'minecraft/font/default.json').write_text(json.dumps(dict(providers=providers), indent=2) + '\n')
for old in ['badge1', 'badge2', 'badge3', 'badge_shield', 'badge_shield_tab', 'badge_shield_tag']:
    for path in [ROOT / (old+'.png'), ROOT / 'font' / (old+'.json'), ROOT / 'textures/font' / (old+'.png')]:
        path.unlink(missing_ok=True)
