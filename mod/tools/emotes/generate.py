"""Rebuild the approved atlas/UV map and Java catalog. Requires Pillow and librsvg/cairo.

Run from any directory: python3 mod/tools/emotes/generate.py
Only the approved, vendored SVGs in manifest.json are used. 128px source cells,
with 8px transparent gutters, keep neighbouring sprites out of filtered quads.
"""
import ctypes as c
import io
import json
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
MOD = HERE.parents[1]
ASSETS = MOD / 'core/src/main/resources/assets/arctic'
manifest = json.loads((HERE / 'manifest.json').read_text())
rsvg = c.CDLL('librsvg-2.so.2')
cairo = c.CDLL('libcairo.so.2')
gobject = c.CDLL('libgobject-2.0.so.0')
rsvg.rsvg_handle_new_from_file.argtypes = [c.c_char_p, c.c_void_p]
rsvg.rsvg_handle_new_from_file.restype = c.c_void_p
rsvg.rsvg_handle_render_document.argtypes = [c.c_void_p, c.c_void_p, c.c_void_p, c.c_void_p]
rsvg.rsvg_handle_render_document.restype = c.c_int
cairo.cairo_image_surface_create.argtypes = [c.c_int, c.c_int, c.c_int]
cairo.cairo_image_surface_create.restype = c.c_void_p
cairo.cairo_create.argtypes = [c.c_void_p]
cairo.cairo_create.restype = c.c_void_p
WRITE = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_void_p, c.c_uint)
cairo.cairo_surface_write_to_png_stream.argtypes = [c.c_void_p, WRITE, c.c_void_p]
for name in ['cairo_destroy', 'cairo_surface_destroy']:
 getattr(cairo, name).argtypes = [c.c_void_p]
gobject.g_object_unref.argtypes = [c.c_void_p]


def raster(path):
    handle = rsvg.rsvg_handle_new_from_file(str(path).encode(), None)
    assert handle, path
    surface = cairo.cairo_image_surface_create(0, 128, 128)
    ctx = cairo.cairo_create(surface)
    viewport = (c.c_double * 4)(0, 0, 128, 128)
    assert rsvg.rsvg_handle_render_document(handle, ctx, viewport, None), path
    data = bytearray()
    @WRITE
    def write(_, buf, size):
        data.extend(c.string_at(buf, size))
        return 0
    assert cairo.cairo_surface_write_to_png_stream(surface, write, None) == 0
    cairo.cairo_destroy(ctx)
    cairo.cairo_surface_destroy(surface)
    gobject.g_object_unref(handle)
    return Image.open(io.BytesIO(data)).convert('RGBA')


atlas = Image.new('RGBA', (1152, 1152))
uv = {'width': 1152, 'height': 1152, 'cell': 144, 'gutter': 8, 'sprites': []}
entries = []
index = 0
for group in manifest['groups']:
    for e in group['emotes']:
        x, y = (index % 8) * 144 + 8, (index // 8) * 144 + 8
        atlas.paste(raster(HERE / e['file']), (x, y))
        uv['sprites'].append(dict(e, group=group['name'], index=index, x=x, y=y, size=128))
        aliases = ', '.join('"' + a + '"' for a in e['aliases'])
        entries.append('\t\tadd("%s", "%s"%s);' % (group['name'], e['code'], ', ' + aliases if aliases else ''))
        index += 1
assert index == manifest['total'] == 64
ASSETS.mkdir(parents=True, exist_ok=True)
atlas.save(ASSETS / 'emotes.png', optimize=True)
(ASSETS / 'emotes.json').write_text(json.dumps(uv, indent=2) + '\n')
java = MOD / 'core/src/main/java/com/arcticlauncher/client/feature/EmoteAtlas.java'
text = java.read_text()
start, end = '// BEGIN GENERATED', '// END GENERATED'
a, tail = text.split(start)
_, b = tail.split(end)
java.write_text(a + start + '\n' + '\n'.join(entries) + '\n\t\t' + end + b)
