# Launcher fallback fonts

- The two DejaVu 2.37 subsets are distributed under [the upstream font licenses](DejaVu-LICENSE.txt). Their internal family names are Arctic Fallback Sans and Arctic Fallback Mono.
- The Noto Sans CJK SC 2.004 subset and Noto Emoji 1.05 are distributed under the [SIL Open Font License](Noto-OFL.txt).
- The [emoji-icon-font](emoji-icon-font-MIT.txt) is distributed under the MIT license. It and Noto Emoji come from `epaint_default_fonts` 0.36.2 and retain the launcher's previous outline emoji repertoire.

To update coverage, run `python tools/font_coverage.py --codepoints > codepoints.txt` from the repository root. Use FontTools `pyftsubset` with `--no-hinting`, `--name-IDs=0,1,2,3,4,5,6,13,14` and these inputs.

| Source | Subset |
| --- | --- |
| DejaVu Sans 2.37 | `--unicodes-file=codepoints.txt` and `--unicodes=U+0020-052F,U+2000-206F,U+2190-21FF,U+2500-257F` |
| DejaVu Sans Mono 2.37 | `--unicodes=U+0020-052F,U+2000-206F,U+2190-21FF,U+2500-257F,U+FFFD` |
| [Noto Sans CJK SC Regular 2.004](https://github.com/notofonts/noto-cjk/blob/Sans2.004/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf) | `--unicodes-file=codepoints.txt` |

For the DejaVu subsets, update name records 1, 3, 4, and 6 to the names above (no spaces in the PostScript name). The build compresses the faces into LZMA blobs. No font tooling is required to build the launcher.
