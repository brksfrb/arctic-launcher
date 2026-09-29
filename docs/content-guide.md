# Making content for Arctic

How to make skins, capes, animated capes, 3D cosmetics and emotes in bulk so they drop
straight into Arctic. Give this whole file to whoever (or whatever AI) is making them.

All content lives in `crates/arctic-cosmetics/assets/`. The cosmetics server checks every file
when it starts, and a broken file stops it with a message naming the file, so a mistake never
reaches players. Pushing to `main` deploys it.

```
assets/
├── catalog.json            capes (still and animated)
├── capes/<id>.png
├── cosmetics.json          3D cosmetics
├── cosmetics/<id>.geo.json
├── cosmetics/<id>.png
├── cosmetics/<id>.animation.json   (optional idle motion)
├── emotes.json
├── emotes/<id>.animation.json
├── gallery.json            skins in the community gallery
└── gallery/<file>.png
```

## Rules for everything

- **ids**: 1–32 characters of `a-z 0-9 _ -`, unique within their list (`frost_wings`, `santa_hat`).
- **names**: what players see, up to 32 characters, Title Case (`Frost Wings`).
- **PNG only**, RGBA, no color profile tricks. Transparent pixels are fully transparent (alpha 0).
- **Original work only.** No copyrighted characters, brand logos or other games' assets. Nothing
  offensive, no real people. Arctic's style is wintry and clean (ice blues, aurora greens and
  purples, white and navy), but anything that fits Minecraft's look is welcome.
- Pixel art at Minecraft's scale: flat colors with a little shading (2–4 tones per material),
  no blur, no anti-aliasing, no photos.

## How many to make

A good first batch, so every section looks full without being overwhelming:

| What | Amount | Notes |
|---|---|---|
| Gallery skins | 40–60 | A mix: about a third slim; themes, jobs, animals, fantasy, winter |
| Still capes | 12–20 | |
| Animated capes | 6–10 | These stand out; keep them tasteful |
| 3D cosmetics | 30–40 | Head 10–12, face 6–8, back 6–8, shoulders 4–6, body 4–6 |
| Emotes | 6 more (12 total) | The emote wheel shows 12 |

Later batches can be any size. The client handles up to 500 cosmetics and 200 capes.

## Skins (gallery)

1. **64×64 PNG** in the standard Minecraft skin layout (the modern one with separate left
   arm/leg and the second "overlay" layer). Legacy 64×32 works but avoid it.
2. **Classic** (4-pixel arms) or **slim** (3-pixel arms). Paint slim skins with the 3-wide arm
   layout, or the arms will show a stripe.
3. Use the overlay layer (hat, jacket, sleeves, pants) for depth: hair, hoods, collars. Keep
   overlay pixels either fully opaque or fully transparent.
4. Save as `gallery/<file>.png` (lowercase, no spaces: `ice_mage.png`), then add an entry:

```json
[
  { "file": "ice_mage.png", "name": "Ice Mage", "model": "slim" },
  { "file": "night_guard.png", "name": "Night Guard", "model": "classic" }
]
```

They show in the gallery as made by **Arctic**, and each is added once (editing the list later
won't duplicate them). The same skin image can't be added twice.

## Capes

A cape is one PNG in Minecraft's cape layout, listed in `catalog.json`:

```json
[
  { "id": "aurora", "name": "Aurora", "free": true },
  { "id": "ember_flow", "name": "Ember Flow", "free": true }
]
```

and saved as `capes/<id>.png`.

### Size and layout

- Make capes **HD**: **128×64** (the back is 20×32 px) for most, **256×128** (40×64 px) for
  showpieces and animated ones. 512×256 also works. Plain 64×32 is vanilla's size and only
  leaves a 10×16 back, too little for a design; don't use it. Width is always twice the height.
- **Real pixel art at the final size**: every pixel placed on purpose. Don't generate a picture
  and shrink it; that gives speckled noise. Use 10–16 colors, clean shapes and outlines, soft
  shading with 2–4 tones per color, and no dithering or random dots.
- **Each cape its own design**: its own border (or none), motif and colors. Don't reuse one
  frame template for every cape.
- Minecraft's cape layout (at 64×32; double every number for 128×64):
  - **back** (what others see): x 1–10, y 1–16 (10×16)
  - **front** (against the player's back): x 12–21, y 1–16
  - **left and right edges**: x 0 and x 11 (1×16 each)
  - **top and bottom edges**: x 1–10 and x 11–20, y 0 (10×1 each)
  - the rest (x 22 and beyond) is the elytra area; paint it to match or leave it transparent
- Paint the edges too, or the cape looks paper-thin from the side.

### Animated capes

- Stack **2 to 8 frames** vertically in one PNG: each frame is a full cape image (height =
  width ÷ 2), so a 6-frame 128×64 cape is **128×384** and an 8-frame 256×128 one is
  **256×1024**. Frames should differ in motion (light moving, snow falling), not just flicker.
- Frames play at **8 per second** (0.125 s each), looping. Make the last frame lead smoothly
  back into the first.
- Keep the motion slow: shimmer, a drifting aurora, falling snow, a pulse. Avoid fast flashing.
- The launcher shows animated capes in their own **Animated** row automatically.

## 3D cosmetics

Made in **Blockbench** as a **Bedrock Entity** model (or written as JSON directly), exported as
Bedrock Geometry plus the texture. The generator in `assets/tools/make_cosmetics.py` shows how
to write them by code.

### Units and where the player is

Units are pixels; 16 = one block. The player stands on y = 0 and faces **−z** (north):

| Part | x | y | z | Bone pivot |
|---|---|---|---|---|
| head | −4 … 4 | 24 … 32 | −4 … 4 | `0, 24, 0` |
| body | −4 … 4 | 12 … 24 | −2 … 2 | `0, 24, 0` |
| rightArm | −8 … −4 | 12 … 24 | −2 … 2 | `−5, 22, 0` |
| leftArm | 4 … 8 | 12 … 24 | −2 … 2 | `5, 22, 0` |
| rightLeg | −4 … 0 | 0 … 12 | −2 … 2 | `−1.9, 12, 0` |
| leftLeg | 0 … 4 | 0 … 12 | −2 … 2 | `1.9, 12, 0` |

The hat layer sits 0.5 px outside the head, so anything worn on the head should start at least
0.5 px out (a hat's brim at y = 32, glasses at z = −4.5 or further forward).

### Bones

- Name each **root bone** after the part it follows: `head`, `body`, `rightArm`, `leftArm`,
  `rightLeg` or `leftLeg`, with the pivot from the table. Anything else follows the body.
- Child bones (with `parent`) can have their own pivot and `rotation` (degrees): use them to
  tilt pieces, like a crooked hat or angled wings. Cubes themselves can't be rotated.
- Limits: 64 bones, 256 cubes, coordinates within ±256, each file up to 256 KB. Keep a
  cosmetic around 5–40 cubes; it's drawn for every player on screen.

### Cubes and the texture (box UV)

- **Box UV only** (Blockbench's default). Each cube gives `origin`, `size` and `uv` (the
  top-left of its area in the texture). Optional: `"inflate"` (grow outward, up to 16) and
  `"mirror": true`.
- A cube of size (w, h, d) takes a `2(w+d)` × `(d+h)` area of the texture starting at `uv`:
  - top: (u+d, v), w×d; bottom: (u+d+w, v), w×d
  - right side: (u, v+d), d×h; front: (u+d, v+d), w×h
  - left side: (u+d+w, v+d), d×h; back: (u+2d+w, v+d), w×h
- Areas must not overlap unless you want two cubes to share paint.
- Texture sides must be **powers of two, up to 512** (16×16, 64×32, 128×64…). Use the smallest
  that fits; 64×32 is typical.
- Use full pixels at the model's scale (1 texel = 1 unit). Don't shrink a 32×32 painting onto
  an 8×8 face.

### Slots

Each cosmetic has one slot, and a player wears **one per slot**:

| Slot | For |
|---|---|
| `head` | hats, crowns, ears, halos, helmets, headphones |
| `face` | glasses, masks, mustaches |
| `back` | wings, backpacks, capes-that-aren't-capes, tails |
| `shoulders` | pets and small things on a shoulder |
| `body` | scarves, belts, necklaces, sashes |

### Idle motion (optional)

For wings flapping, a tail swaying or a pet bobbing: in Blockbench's Animate tab make **one
looping animation** of your bones, export Bedrock Animations as `cosmetics/<id>.animation.json`.
Rotation and position keyframes are used, blended linearly; expressions (molang) count as 0.
Keep it gentle and 1–4 seconds long.

### Listing it

```json
{ "id": "ice_crown", "name": "Ice Crown", "slot": "head" }
```

in `cosmetics.json`, with `cosmetics/ice_crown.geo.json` and `cosmetics/ice_crown.png`.

### Checklist before handing one over

- Seen from the front, side and back, nothing pokes into the head or body (the launcher
  preview and the game draw it exactly where the numbers say).
- Every visible face is painted (no transparent or default-grey faces).
- It reads at small size: bold shapes and 2–4 colors beat detail.

## Emotes

Whole-player animations played from the emote wheel.

1. Blockbench: the player template as a Bedrock Entity (bones `head`, `body`, `rightArm`,
   `leftArm`, `rightLeg`, `leftLeg`, pivots as in the table above).
2. Animate tab: **one animation, up to 15 seconds**. Rotations replace the walking pose of the
   bones you animate; positions add to them. Set it to loop for emotes that last until the
   player moves (a dance), or not (a wave, a bow).
3. Export Bedrock Animations as `emotes/<id>.animation.json` and add
   `{ "id": "salute", "name": "Salute" }` to `emotes.json`.

Short emotes (1–3 s) with a clear pose read best. Moving always stops an emote.

## File formats in short

Geometry (`cosmetics/<id>.geo.json`):

```json
{
  "format_version": "1.12.0",
  "minecraft:geometry": [{
    "description": { "identifier": "geometry.ice_crown", "texture_width": 64, "texture_height": 16 },
    "bones": [{
      "name": "head",
      "pivot": [0, 24, 0],
      "cubes": [
        { "origin": [-4, 32, -5], "size": [8, 2, 1], "uv": [0, 0] }
      ]
    }]
  }]
}
```

Animation (`*.animation.json`):

```json
{
  "format_version": "1.8.0",
  "animations": {
    "animation.ice_wings.idle": {
      "loop": true,
      "animation_length": 2.0,
      "bones": {
        "left_wing": { "rotation": { "0.0": [0, -10, 0], "1.0": [0, 15, 0], "2.0": [0, -10, 0] } }
      }
    }
  }
}
```

## Handing it over

Deliver a folder with the same layout as `assets/` (only the new files) plus the new entries for
`catalog.json`, `cosmetics.json`, `emotes.json` and `gallery.json`. Before pushing, run the
server's checks locally:

```bash
cargo test -p arctic-cosmetics
```

A problem shows as a failing test naming the file and what's wrong.
