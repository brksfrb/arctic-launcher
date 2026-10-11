# Request: elytra art for every Arctic cape

> **Done (2026-10-11):** every cape and still image in the catalog now has wing art in every frame. This page stays as the spec for new capes. The stopgap issue below is fixed: auto wings put the cape's front on the outside, and frames are checked one by one.

## Why

Minecraft hides the cape while a player wears an elytra and draws the elytra from the **same cape
image** instead, using the wing area to the right of the cape (x 22–45 in 64-wide units). Only 5 of
the 39 Arctic capes have anything painted there, and those 5 only have a plain gradient (see
[Current state](#current-state)). Without wing art, a player who equips an elytra would lose the cape
look entirely.

As a stopgap the client now paints wings itself when a cape's wing area is empty
(`mod/core/src/main/java/com/arcticlauncher/client/looks/CapeFormat.java`): it stretches the cape's
10×16 faces onto the 10×20 wing faces. That only copies the cape, distorted, and has no wing shape:

- the motif is squashed and repeated on both wings, and it reads as a rectangle, not a wing;
- nothing is painted on the wing edges;
- see [Known issue in the stopgap](#known-issue-in-the-stopgap): it also puts the cape's art on the
  wrong side of the wing.

Designed art replaces the stopgap. **If any pixel in a frame's wing box (x 22–45, y 0–21 in 64-wide
units) is not fully transparent, the client uses the image exactly as painted.** The client only checks
frame 0 of a strip, so every frame must be painted (see [Animated capes](#animated-capes)).

**Art delivered (2026-10-11):** wing art has now been packed into all 39 catalog capes, all eight
runtime `-still.png` fallbacks, and every 512/1024 review atlas in
[`cape-art-pack`](cape-art-pack/README.md). The original cape UV pixels, dimensions, and frame
counts are unchanged. The catalog and client code were not changed by the art pass. The painted
wing faces and edges are present in every animation frame. See the
[`elytra-review.png`](cape-art-pack/elytra-review.png) contact sheet and the packing script in the
art pack. In-game angle and flight review remains to be done on a running client.

## How the game draws the elytra

The layout below comes from vanilla `ElytraModel` (`net.minecraft.client.model.object.equipment` in
26.x, `net.minecraft.client.model` in 1.20.1; the same numbers since elytras were added in 1.9):

```
left_wing : texOffs(22, 0).addBox(-10, 0, 0,  10, 20, 2, inflate 1.0)  pivot ( 5, 0, 0), rot x +15°, z -15°
right_wing: texOffs(22, 0).mirror().addBox(0, 0, 0, 10, 20, 2, inflate 1.0)  pivot (-5, 0, 0), rot x +15°, z +15°
texture size 64×32; layer is moved 2 px behind the body (translate z +0.125)
render type: armorCutoutNoCull (alpha cut out, both sides of every face drawn)
```

Both wings read the **same** pixels. The right wing is the left wing mirrored, so you paint **one
wing** and the game makes the symmetrical pair. There are no separate left and right wing areas.

Elytras exist from Minecraft 1.9. On 1.8.9 the wing area is never drawn, so the cape art itself is
unaffected by anything painted there.

### The wing box at 64×32 (one frame)

One wing is a 10×20×2 box unwrapped like any Minecraft box. In 64-wide pixel units:

```
 x:  22 23 24 ................ 33 34 35 36 ................ 45
y 0  .. .. [ top edge 10×2       ] [ tip edge 10×2        ] .. ..
y 1  .. .. [ (rows 0-1)          ] [ (rows 0-1)           ] .. ..
y 2  F  F  [                     ] H  H  [                       ]
 .   r  r  [ INNER face          ] i  i  [ OUTER face            ]
 .   e  e  [ 10×20               ] n  n  [ 10×20                 ]
 .   e  e  [ against the back    ] g  g  [ what others see       ]
y21  .  .  [                     ] e  e  [                       ]
```

| Part | x | y | Size | Faces | What it is |
| --- | --- | --- | --- | --- | --- |
| **Outer face** | 36–45 | 2–21 | 10×20 | away from the player (+z) | The side everyone sees from behind and when gliding from above. The main art goes here. |
| **Inner face** | 24–33 | 2–21 | 10×20 | toward the player's back (−z) | The underside. Seen from the front and when gliding from below. |
| **Hinge edge** | 34–35 | 2–21 | 2×20 | toward the shoulder | The edge at the shoulder pivot (the wing's leading edge). x 34 joins the inner face, x 35 joins the outer face. |
| **Free edge** | 22–23 | 2–21 | 2×20 | toward the spine | The other long edge. x 22 joins the outer face, x 23 joins the inner face. |
| **Top edge** | 24–33 | 0–1 | 10×2 | up | Top of the wing at shoulder height. Row 0 is on the outer side, row 1 on the inner side. x 24 is at the free edge, x 33 at the hinge. |
| **Tip edge** | 34–43 | 0–1 | 10×2 | down | Bottom (tip) of the wing. Row 0 is on the outer side, row 1 on the inner side. x 34 is at the free edge, x 43 at the hinge. |
| unused | 22–23 and 44–45 | 0–1 | 2×2 each | — | Inside the box the client checks, but never drawn. Leave transparent. |

Each face runs top to bottom: **row 2 is the top of the wing** (shoulder height) and **row 21 is the
tip**.

### Which way round each face reads

| Face | Column next to the shoulder (hinge) | Column next to the spine (free edge) | Seen as painted from |
| --- | --- | --- | --- |
| Outer (36–45) | **x 36** (left) | x 45 (right) | behind, on the player's **left** wing. The right wing shows it mirrored. |
| Inner (24–33) | **x 33** (right) | x 24 (left) | the front, on the player's left wing. The right wing shows it mirrored. |

So a column on the inner face lines up with the opposite column on the outer face: **inner x (24 + k)
sits directly behind outer x (45 − k)**, same row. Vanilla's own `elytra.png` follows this: its
feathered wing is painted only on the outer face, starting narrow at the upper left (the shoulder)
and widening toward the tip, and the inner face is left transparent.

### Rendering details that matter for the art

- **Alpha is on or off.** The elytra uses a cutout render type: a pixel is either drawn or discarded
  (nearly transparent pixels disappear). Don't use soft or partial alpha. Use it to cut a wing
  silhouette, the way vanilla does.
- **No back-face culling.** A face is visible from both sides. Where the inner face is transparent,
  the outer face's art shows through from the front (mirrored), and the other way round. If you cut a
  silhouette, cut **the same shape on both faces** (mirrored as in the column rule above), or the
  inner art will peek out of the holes in the outer face.
- **Slight stretch.** The wing is inflated by 1 unit on every side, so the 10×20 face covers 12×22
  units: pixels draw about 1.2× wider and 1.1× taller than on the cape. Avoid motifs that only work
  when the pixels are exactly square, such as small circles.
- The wings tilt outward at 15° when standing and swing wide and flat when gliding, so the outer face
  is seen almost straight on from behind and from above.

## Coordinates per cape size

Arctic capes scale the 64×32 layout. Multiply every number above by **s = image width ÷ 64**; a part
at (x, y, w, h) covers pixels x·s … (x + w)·s − 1 and y·s … (y + h)·s − 1. For frame *f* (counting from
0) of a vertical strip, add **f × (width ÷ 2)** to every y.

| Image width | s | Whole wing box | Outer face | Inner face | Hinge edge | Free edge | Top edge | Tip edge |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 64 | 1 | 22,0 24×22 | 36,2 10×20 | 24,2 10×20 | 34,2 2×20 | 22,2 2×20 | 24,0 10×2 | 34,0 10×2 |
| 128 | 2 | 44,0 48×44 | 72,4 20×40 | 48,4 20×40 | 68,4 4×40 | 44,4 4×40 | 48,0 20×4 | 68,0 20×4 |
| 256 | 4 | 88,0 96×88 | 144,8 40×80 | 96,8 40×80 | 136,8 8×80 | 88,8 8×80 | 96,0 40×8 | 136,0 40×8 |
| 512 | 8 | 176,0 192×176 | 288,16 80×160 | 192,16 80×160 | 272,16 16×160 | 176,16 16×160 | 192,0 80×16 | 272,0 80×16 |
| 1024 | 16 | 352,0 384×352 | 576,32 160×320 | 384,32 160×320 | 544,32 32×320 | 352,32 32×320 | 384,0 160×32 | 544,0 160×32 |

(x,y w×h in image pixels, frame 0.) For example, frame 3 of a 1024-wide strip has its outer face at
576, 32 + 3×512 = 576,1568, 160×320.

At 1024 wide each wing face is **160×320**, bigger than the cape's own 160×256 back. Paint it at that
size, with the same care as the cape: don't enlarge a small wing.

## Animated capes

- **Every frame needs its own wing art.** The client looks only at frame 0 to decide whether a cape
  has wing art. If frame 0 is painted and a later frame is not, that frame draws no wings at all, so
  the elytra flickers out once per loop.
- The wings play at the cape's `fps` with the same frame count. Carry the cape's motion onto the
  wings (the aurora drifts across the wing, the snow keeps falling) and make the last frame lead back
  into the first just as the cape does.
- Keep the **silhouette identical in every frame** unless the change is the point of the design.
  A wing outline that jumps between frames looks like a glitch.
- **Still images too.** Every `<id>-still.png` is a separate image with its own wing box and gets its
  own check. Paint its wings to match the still cape; a still cape with empty wings falls back to the
  stopgap while its animated version has designed wings.
- **File size.** Capes may be at most 2 MiB. The two 24-frame 1024-wide strips are already about
  1.6 MB (`abyss_lantern_jelly.png` 1,654,838 bytes, `eclipse_orrery.png` 1,611,239 bytes), so their
  wings have roughly 400 KB to fit in. Use flat color areas and few colors on the wings of those
  two; check the final byte size.

## Design guidance

- **A matched set.** The wings should look like they belong to the cape: same palette, same outline
  color, same light direction, a motif taken from the cape (a phoenix cape gets feathered flame wings,
  a circuit cape gets circuit traces along the wing). Don't paste the cape's back onto the wings; the
  shape is different (10×20, tall and narrow, seen in a V from behind) and needs its own composition.
- **Read as wings.** Give the outer face a wing silhouette with alpha (narrow at the shoulder, x 36
  side of row 2, widening toward the tip, with feather, scale, crystal or fin shapes along the free
  edge as fits the motif). A full rectangle reads as two boards on the back.
- **Both faces.** Paint the outer face as the main art and the inner face as a darker or plainer
  underside in the same colors, with the mirrored silhouette. Leaving the inner face transparent is
  allowed (vanilla does it): the outer art then shows through mirrored from the front.
- **Edges.** Paint the hinge, free, top and tip edges in the colors of the faces they join, so the wing
  doesn't look paper-thin from the side. Where the silhouette cuts away part of a face, the edge
  strips along that part can be left transparent.
- **Symmetry is automatic.** The right wing is the mirror of the left. Asymmetric designs (a moon on
  one wing only) are not possible; text or numbers will read backwards on the right wing.
- **Transparency outside.** Everything in the wing box that is not part of the wing must be fully
  transparent (alpha 0), including the two unused 2×2 corners. Everything outside the wing box and the
  cape parts stays transparent as before.
- **Same pixel-art rules as the capes** ([content guide](../content-guide.md), cape section): real
  pixel art at the final size, 10–16 colors, 2–4 tones per color, no dithering or noise, no
  generated picture shrunk down.
- **Don't touch the cape parts.** Only the wing box changes; the cape faces and edges must stay
  pixel-identical.

## Acceptance checks

1. Every catalog cape and every `-still.png` has at least one visible pixel in the wing box of
   **every frame**, and the cape parts (x 0–21, y 0–16 per 64-wide frame) are pixel-identical to the
   current file.
2. Image size, frame count and frame height are unchanged; each file is still at most 2 MiB.
3. Alpha in the wing box is only 0 or 255. The unused corners (22–23 and 44–45, rows 0–1) are alpha 0.
4. The silhouettes of the two faces match: inner pixel (24 + k, y) is opaque exactly when outer pixel
   (45 − k, y) is, unless the design deliberately leaves the inner face transparent.
5. In game (1.9+, both the Fabric and the legacy adapter), with the cape equipped and an elytra worn:
   from behind, three-quarter, the side, the front, and while gliding seen from above and below, the
   wings are recognizably the cape's design, symmetrical, with no stretched copy of the cape and no
   holes showing a different face.
6. Animated capes: wings animate with the cape at its `fps`, never vanish on any frame, and loop
   without a jump. With **Animate** off, the still image shows matching still wings.

A quick check for 1–3 (Pillow): for each frame, crop the wing box at the cape's scale and test
`any(a > 0)`, `set(alpha) <= {0, 255}`, and compare the cape region with the previous file.

## Before the art pass

Checked on 2026-10-11 with Pillow over `crates/arctic-cosmetics/assets/capes/*.png`: "painted" means
some pixel with alpha > 0 in frame 0's wing box. The 10 designs from the
[cape art pack](cape-art-pack/README.md) are all in the catalog (ids with underscores) and are
included below.

- **39 capes**: 5 have wing pixels, **34 have none** (all frames empty, and all 8 still images empty).
- The 5 painted ones (`aurora`, `snowflake`, `glacier`, `midnight`, `borealis`) fill the **entire**
  24×22 box, unused corners included, with a plain vertical gradient in the cape's background color:
  no motif, no silhouette, the same art on both faces. They work as wings only in the sense that they
  stop the stopgap. They need designed art like the rest.

| id | Name | Size (per frame) | Frames | Still image | Wing area painted | Motif suggestion |
| --- | --- | --- | --- | --- | --- | --- |
| `eclipse_orrery` | Eclipse Orrery | 1024×512 | 24 (12 fps) | yes, empty | no | Brass orbit rings sweeping along each wing, light arc traveling down the feathers |
| `abyss_lantern_jelly` | Abyss Lantern Jelly | 1024×512 | 24 (12 fps) | yes, empty | no | Translucent-looking jellyfish bell wings with glowing trailing tentacle edges, rising bubbles |
| `koi_current` | Koi Current | 1024×512 | 16 (8 fps) | yes, empty | no | Koi fin wings: flowing fin rays with scale shimmer, ripple carried outward |
| `moon_kitsune` | Moon Kitsune | 1024×512 | 16 (8 fps) | yes, empty | no | Fox-tail wings in white and moon-blue, petals drifting down them |
| `celestial_moth` | Celestial Moth | 1024×512 | 16 (8 fps) | yes, empty | no | Moth wings with eye spots and moving starlight dust |
| `eternal_hourglass` | Eternal Hourglass | 1024×512 | 16 (8 fps) | yes, empty | no | Brass-ribbed wings with sand streaming toward the tips |
| `aurora_wolf` | Aurora Wolf | 1024×512 | 16 (8 fps) | yes, empty | no | Wolf-fur feathered wings under a traveling aurora band |
| `frostblade` | Frostblade | 1024×512 | 16 (8 fps) | yes, empty | no | Ice-blade feathers, sharp crystalline free edge with climbing glints |
| `solar_phoenix` | Solar Phoenix | 1024×512 | 1 | — | no | Flame-feather phoenix wings, gold at the shoulder to red at the tips |
| `jade_dragon` | Jade Dragon | 1024×512 | 1 | — | no | Jade dragon wings: membrane between finger bones, scaled hinge edge |
| `aurora` | Aurora | 128×64 | 1 | — | yes (plain gradient) | Night-blue wings with green-to-violet aurora bands across them |
| `snowflake` | Snowflake | 128×64 | 1 | — | yes (plain gradient) | Navy wings framed in ice-blue, one snowflake per wing |
| `glacier` | Glacier | 128×64 | 1 | — | yes (plain gradient) | Pale ice wings with blue crack lines running to the tips |
| `midnight` | Midnight | 128×64 | 1 | — | yes (plain gradient) | Dark bat-like wings dotted with stars, crescent at the shoulder |
| `borealis` | Borealis | 128×64 | 6 | — | yes, all 6 frames (plain gradient) | Northern-light ribbons flowing along the wings, matching the cape's motion |
| `aurora_veil` | Aurora Veil | 128×64 | 1 | — | no | Sheer veil-like wings with soft aurora stripes |
| `glacier_crest` | Glacier Crest | 128×64 | 1 | — | no | Ice-ridge wings with a jagged crest along the free edge |
| `snowfall_bloom` | Snowfall Bloom | 128×64 | 1 | — | no | Petal-shaped wings with snow-dusted blossoms |
| `polar_horizon` | Polar Horizon | 128×64 | 1 | — | no | Horizon gradient wings: pale sky above, white ice line, deep sea at the tips |
| `moonlit_pines` | Moonlit Pines | 128×64 | 1 | — | no | Night wings with a pine-tree silhouette edge along the tips |
| `crystal_shard` | Crystal Shard | 128×64 | 1 | — | no | Faceted crystal-shard wings with light facets |
| `copper_compass` | Copper Compass | 128×64 | 1 | — | no | Copper-plated wings with rivets and compass-needle points |
| `ember_forge` | Ember Forge | 128×64 | 1 | — | no | Iron-plate wings with glowing ember seams |
| `tidal_wave` | Tidal Wave | 128×64 | 1 | — | no | Curling wave wings with foam along the free edge |
| `moss_rune` | Moss Rune | 128×64 | 1 | — | no | Stone wings with moss and one glowing rune each |
| `royal_amethyst` | Royal Amethyst | 128×64 | 1 | — | no | Purple velvet wings trimmed in gold, amethyst at the shoulder |
| `cloud_sailing` | Cloud Sailing | 128×64 | 1 | — | no | Sail-cloth wings with puffy cloud tips |
| `starlight_patchwork` | Starlight Patchwork | 128×64 | 1 | — | no | Stitched patchwork wings, star-pattern patches |
| `raven_wing` | Raven Wing | 128×64 | 1 | — | no | Glossy black raven feathers with a blue-violet sheen |
| `cobalt_circuit` | Cobalt Circuit | 128×64 | 1 | — | no | Cobalt panel wings with circuit traces running to the tips |
| `sunrise_peak` | Sunrise Peak | 128×64 | 1 | — | no | Sunrise-gradient feathers, mountain silhouette along the tips |
| `living_aurora` | Living Aurora | 256×128 | 8 | — | no | Aurora curtains rippling down the wings with the cape's motion |
| `quiet_snow` | Quiet Snow | 256×128 | 8 | — | no | Soft grey-blue wings with snow falling over them |
| `glacial_heartbeat` | Glacial Heartbeat | 256×128 | 8 | — | no | Ice wings with a pulse of light spreading from the shoulder |
| `starlit_orbit` | Starlit Orbit | 256×128 | 8 | — | no | Deep-space wings with a star circling along the edge |
| `ember_breath` | Ember Breath | 256×128 | 8 | — | no | Charred wings with embers breathing brighter and drifting up |
| `ocean_current` | Ocean Current | 256×128 | 8 | — | no | Sea-blue fin wings with currents flowing toward the tips |
| `rune_awaken` | Rune Awaken | 256×128 | 8 | — | no | Dark stone wings whose runes light up in sequence |
| `prism_shimmer` | Prism Shimmer | 256×128 | 8 | — | no | Glassy wings with a rainbow shimmer sweeping across |

No catalog cape is 512 wide today; the 512 row in the coordinates table is for new capes and for the
512-wide versions in the cape art pack.

## Known issue in the stopgap

The stopgap in `CapeFormat.java` copies the cape's outer art (64-unit 1,1 10×16) onto **24,2** and
the cape's inner side (12,1) onto **36,2**. By the vanilla model above, 36,2 is the outer face that
others see and 24,2 the underside, so the auto-painted wings currently show the cape's plain inner
side from behind and put the motif underneath. Designed art avoids this entirely, but until every
cape has it, the stopgap's two target rectangles should be swapped (outer art to 36,2, inner to
24,2). That is a code change and is not part of this art request.
