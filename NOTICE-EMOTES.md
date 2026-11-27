The 64 chat emotes in `mod/tools/emotes/manifest.json` are proposal 2, approved
by Burak on 2026-11-10. The manifest, vendored SVG sources, generated atlas and
UV map preserve that set and its primary shortcodes/aliases.

62 common glyphs: Twemoji, Copyright Twitter, Inc. and other contributors.
Graphics licensed under Creative Commons Attribution 4.0 International:
https://creativecommons.org/licenses/by/4.0/
Project: https://github.com/twitter/twemoji
The graphics were rasterized and arranged in an atlas; their artwork was not restyled.

`:arctic:` is the official Arctic mark, in brand blue #2EB1EC.
`:ice:` is Arctic's custom crystal. These two are not Twemoji graphics.

Rebuild with `python3 mod/tools/emotes/generate.py` (Python Pillow and system
librsvg/cairo). Runtime uses one RGBA atlas and the generated catalog/UV map.
Shortcodes stay readable on the wire. A disposable chat layout copy uses private
glyphs for line breaking, widths, hit testing and RGBA textured quads. Original
messages and source copy metadata retain the shortcode text. Both adapters draw textured quads directly from the same registered atlas; modern
font glyph metrics and quads reference it without allocating separate glyph textures.
