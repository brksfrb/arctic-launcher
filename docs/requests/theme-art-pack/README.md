# Theme art pack: brief for an image model

Arctic's menus and the launcher need real artwork instead of the procedural gradients they
draw now. This file is the exact brief: what to make, the file names and sizes the renderer
will expect, and the animation each layer supports.
Design background: [`docs/design/ui-redesign-2026-10.md`](../../design/ui-redesign-2026-10.md), section 3.

## Style
- Calm, cold, atmospheric. Arctic night, northern lights, snow, ice, distant mountains.
- Painterly or soft 3D, not pixel art, not photoreal. Wide, quiet, with an empty centre
  (menus sit on top of it) and the detail near the top and bottom edges.
- No text, no logos, no characters, no UI.
- Must read well with dark translucent panels over it: keep the middle band (35% to 70% of
  the height) low contrast.

## Themes to make (5)

| id | mood | palette hint |
|---|---|---|
| `aurora-night` | the signature look: green and violet lights over snowy ridges | navy `#0b1320`, cyan `#7dd3fc`, green `#34d399`, violet `#a78bfa` |
| `glacier-dawn` | pale pink and gold sunrise on blue ice | slate `#13202e`, ice `#bfe3f5`, rose `#f4b6c2` |
| `deep-frost` | underwater-blue ice cave, light shafts, bubbles | `#07131c`, teal `#2dd4bf`, white `#e6fbff` |
| `ember-cold` | a campfire glow far away in a snowy forest at dusk | `#140f14`, amber `#fbbf24`, blue `#60a5fa` |
| `stormfront` | heavy grey-blue clouds over a frozen sea, lightning far off | `#0d1117`, steel `#94a3b8`, white `#f1f5f9` |

## Layers per theme (file names the renderer loads)
All images **16:9, 1920x1080**, WebP (PNG accepted), sRGB. Layers with transparency use a real
alpha channel.

| file | content | transparency | animation the renderer applies |
|---|---|---|---|
| `sky.webp` | the full sky and distant background, no foreground | opaque | none (can be parallax) |
| `lights.webp` | the aurora / glow / light shafts only, on black or transparent | alpha or black (added) | `drift` slow sideways + `pulse` |
| `stars.webp` | stars / snow sparkle / bubbles only | alpha | `twinkle` |
| `far.webp` | far ridge or far shapes | alpha | `parallax` (a few pixels) |
| `near.webp` | the near foreground (trees, ice, rocks) along the bottom third | alpha | `parallax` (a few more pixels) |
| `thumb.webp` | 480x270 preview of the finished composite | opaque | none |

Tips for generation:
- Make `lights.webp` wider than the frame if possible (2560x1080) so a slow sideways drift never
  shows an edge; if not, make it **tile horizontally** (left edge matches right edge).
- Generate the layers as separate images on the same camera, not as one image cut apart:
  ask for "the same scene, sky only", then "only the aurora on black", and so on.
- Keep each file under 400 KB (WebP quality 80 to 85).

## Optional: short loops (only if easy)
`lights-loop.webp`: an animated WebP, 12 frames at 6 fps, 960x540, seamless loop. Used instead
of `lights.webp`'s drift when present.

## Delivery
Put each theme in `crates/arctic-cosmetics/assets/themes/<id>/` (same folder layout the
cosmetics server already uses for capes), plus a one-line `credit.txt` per theme (tool and
date). I wire them into `theme.json` and the launcher; no code changes are needed on your side.
