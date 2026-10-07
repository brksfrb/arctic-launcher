# Making content for Arctic

How to make skins, capes, animated capes, 3D cosmetics and emotes in bulk so they drop
straight into Arctic. Give this whole file to whoever (or whatever AI) is making them.

All content lives in `crates/arctic-cosmetics/assets/`. **For 3D cosmetics and emotes, start from the
kit in `assets/tools/` (see those sections): it encodes the format so the result loads and looks right.** The cosmetics server checks every file
when it starts, and a broken file stops it with a message naming the file, so a mistake never
reaches players. Pushing to `main` deploys it.

```
assets/
├── catalog.json            capes (still and animated)
├── capes/<id>.png
├── cosmetics.json          3D cosmetics
├── cosmetics/<id>.geo.json
├── cosmetics/<id>.png
├── cosmetics/<id>.glow.png         (optional, parts that shine; same size as the png)
├── cosmetics/<id>.animation.json   (optional idle motion)
├── emotes.json
├── emotes/<id>.animation.json
├── gallery.json            skins in the community gallery
├── gallery/<file>.png
└── tools/                  cosmetic_kit.py, preview_cosmetic.py, examples (start here)
```

## Rules for everything

- **ids**: 1–32 characters of `a-z 0-9 _ -`, unique within their list (`frost_wings`, `ice_crown`).
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
| 3D cosmetics | 30–40 | Head 10–12, face 6–8, back 6–8 (wings first), shoulders 4–6, body 4–6. Each at the quality bar below |
| Emotes | 20–30 | Replace the current 12; see the emote quality bar |

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

### Variety (don't make one skin 40 times)

A gallery where every skin is "a person in a blue-ish outfit" looks like one skin recolored.
Spread a batch across:

- **Who**: different skin tones, ages, hair styles and lengths, beards, glasses, and faces
  with different expressions. Not every character is a young adult with brown hair.
- **What**: jobs (chef, pilot, knight, scientist, farmer), fantasy (wizard, elf, robot,
  vampire), animals and mob-like creatures (fox, penguin, frog, axolotl), casual outfits
  (hoodie, streetwear, pajamas), seasonal (winter, beach, Halloween).
- **Palette**: each skin gets its own 2–3 main colors. Across the batch use every hue, not
  only blues and teals. Warm, dark, pastel and bright skins should all be there.
- **Silhouette**: hoods, hats, big hair, helmets, capes-in-the-skin, armor shoulders (all in
  the overlay layer) so skins differ even as thumbnails.

Quality: shade every body part (3–4 tones, light from above), give clothes folds and seams,
faces clear eyes that read at a distance. No noise textures and no smooth gradients.

## Capes

A cape is one PNG in Minecraft's cape layout, listed in `catalog.json`:

```json
[
  { "id": "aurora", "name": "Aurora", "free": true },
  { "id": "koi_current", "name": "Koi Current", "free": true, "fps": 12 }
]
```

and saved as `capes/<id>.png`. An animated cape may also have `capes/<id>-still.png` (one frame, the
same width): its **still image**, worn when the player turns **Animate** off for that cape. `fps` (1 to 30,
default 8) is how fast the frames play.

### Size and layout

- Make capes **HD**: **128×64** (the back is 20×32 px) for simple ones, **512×256** (80×128 px)
  or **1024×512** (160×256 px) for detailed art. 1024 wide is the maximum. Plain 64×32 is vanilla's
  size and only leaves a 10×16 back, too little for a design; don't use it. Width is always twice
  the height. Paint at the final back-panel size (80×128 at 512, 160×256 at 1024): enlarging a small
  motif does not make an HD cape.
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

- Stack **2 to 32 frames** vertically in one PNG: each frame is a full cape image (height =
  width ÷ 2) and the whole strip is at most **12288 pixels tall**. So 1024×512 frames: up to
  **24** (1024×12288); 512×256 frames: up to **32** (512×8192); a 6-frame 128×64 cape is 128×384.
  The PNG file may be up to **2 MiB**. Frames should differ in motion (light moving, snow
  falling), not just flicker.
- Frames play at the cape's `fps` (default **8** per second), looping. Make the last frame lead
  smoothly back into the first, and keep a traveling motion going the same way across the loop.
  12-16 fps with 16-32 frames gives a smoother 2-3 second loop.
- Keep the motion slow: shimmer, a drifting aurora, falling snow, a pulse. Avoid fast flashing.
- Give every animated cape a painted **still image** (`<id>-still.png`, one frame, same width): the
  wearer's **Animate** switch (in the launcher's cape cards and in the game's Looks menu) shows it to
  everyone, and older clients wear it as an ordinary cape. A viewer can also turn on **Freeze animated
  capes** to see every animated cape standing still; that never changes what its wearer chose.
- The launcher shows animated capes in their own **Animated** row automatically.
- Memory: an animated 1024×512 cape costs about 2 MB per frame on the GPU (32 MB for 16 frames);
  prefer 512×256 unless the extra detail is visible.

## 3D cosmetics

**Use the kit.** `assets/tools/cosmetic_kit.py` builds a cosmetic in code with every convention below
already handled (texture packing, face orientation, rotation signs, glow, animation, limits), and
`assets/tools/preview_cosmetic.py` renders it from the back, three-quarter and side. The worked example
`assets/tools/example_angel_wings.py` is the quality bar: copy it, change the shapes and colors.

```bash
python crates/arctic-cosmetics/assets/tools/example_angel_wings.py
python crates/arctic-cosmetics/assets/tools/preview_cosmetic.py crates/arctic-cosmetics/assets/tools/out/angel_wings angel_wings
```

You can also make them in **Blockbench** as a **Bedrock Entity** model, exported as Bedrock Geometry
plus the texture (per-face UV and cube rotation are Blockbench's normal export). The rest of this
section is the exact format either way.

### The idea: paint shapes, don't stack boxes

Vanilla-style cosmetics are boxes with 1 texel per unit, which is why they look blocky. Good ones are
**flat painted planes with a cut-out silhouette**, put together in layers:

- A **card** is a thin cube (0.25 thick) that shows a painted picture on its back and front. Transparent
  pixels are not drawn, so a feather, a leaf, a flame, a rune circle, a ribbon or a halo is just a
  card with the right picture.
- The texture can be **much finer than the model**: a face 28 units long can map onto 224 texels
  (8 texels per unit). Detail comes from the paint.
- Cards are **tilted and fanned** with cube rotation, and layered a fraction of a unit apart.
- **Glow** (below) makes eyes, gems, runes and feather tips shine in the dark.
- Boxes still have a place (a backpack body, a gem, a headphone cup), usually with rotation and
  painted faces.

### Units and where the player is

Units are pixels; 16 = one block. The player stands on y = 0 and faces **−z** (north). **+x is the
player's left**, +y is up, **+z is behind the player**.

| Part | x | y | z | Bone pivot |
|---|---|---|---|---|
| head | −4 … 4 | 24 … 32 | −4 … 4 | `0, 24, 0` |
| body | −4 … 4 | 12 … 24 | −2 … 2 | `0, 24, 0` |
| rightArm | −8 … −4 | 12 … 24 | −2 … 2 | `−5, 22, 0` |
| leftArm | 4 … 8 | 12 … 24 | −2 … 2 | `5, 22, 0` |
| rightLeg | −4 … 0 | 0 … 12 | −2 … 2 | `−1.9, 12, 0` |
| leftLeg | 0 … 4 | 0 … 12 | −2 … 2 | `1.9, 12, 0` |

The hat layer sits 0.5 px outside the head, so anything worn on the head should start at least 0.5 px
out. Wings and backpacks start at z = 2.2 or more (the body's back is at z = 2).

### Bones

- Name each **root bone** after the part it follows: `head`, `body`, `rightArm`, `leftArm`, `rightLeg`
  or `leftLeg`, with the pivot from the table. Anything else follows the body.
- Child bones (`parent`) have their own `pivot` and `rotation` (degrees). The animation moves bones,
  so put things that move together (one wing) in one bone.
- **Rotation order is Z, then Y, then X**, around the pivot, for bones and cubes alike. Rules of thumb:
  on a thing lying along +x, `z = +30` tips its far end 30° **down**, `z = −30` tips it up; `y = −10`
  swings a +x tip 10° backward (toward +z); on the player's right side (−x) the signs flip.
- Limits: 128 bones, 512 cubes, coordinates within ±256, each file up to 512 KB. It is drawn for every
  player on screen: **aim for 20–120 cubes**; many small cards are fine, hundreds of boxes are not.

### Cubes and texture rectangles (per-face UV)

A cube gives `origin` (its minimum corner), `size` (x, y, z), optional `rotation` (degrees) with
`pivot` (the point it turns around; the cube's middle if left out), optional `inflate` (grow outward,
up to 16), and its texture, one of two ways:

**Per face (use this):** one rectangle per face you want drawn. Faces you leave out are not drawn.

```json
{ "origin": [2.6, 18, 2.3], "size": [30, 6.5, 0.25],
  "pivot": [2.6, 21, 2.3], "rotation": [0, 0, 20],
  "uv": {
    "south": { "uv": [0, 0],  "uv_size": [240, 52] },
    "north": { "uv": [240, 0], "uv_size": [-240, 52] } } }
```

- `uv` is the rectangle's top-left corner and `uv_size` its width and height, **in texels of the
  texture** (not model units). A negative size flips that direction.
- Faces: **north** = the front (−z), **south** = the back (+z, what other players see most),
  **east** = the player's right side (−x), **west** = the player's left side (+x), **up**, **down**.
  The top-left corner of a face is as seen from **outside** the cube (for up/down: seen from above/below
  with north at the top).
- A flat card shows `south` to someone behind the player and `north` to someone in front. Give a card
  both; with the same rectangle both read upright (the front shows it as if through the card). The
  kit's `card()` does this and has `mirror=True` for the other wing.

**Box UV (old way, still works):** `"uv": [u, v]` gives the usual six rectangles around (u, v): a cube
of size (w, h, d) takes a `2(w+d)` × `(d+h)` area: top at (u+d, v), bottom (u+d+w, v), east (u, v+d),
north (u+d, v+d), west (u+d+w, v+d), south (u+2d+w, v+d). `"mirror": true` flips it sideways. Per-face
UV with those same rectangles is identical.

### The texture

- PNG, RGBA, **power-of-two** sides up to **1024**; `texture_width` / `texture_height` in the geometry
  **must equal the PNG's size** (the server refuses a mismatch) and every rectangle must lie inside it.
- Paint at **4–8 texels per model unit** for anything people look at (wings, capes of 3D, halos).
  Clean pixel art: no blur, no noise, 3–5 tones per material, a 1-texel darker outline for a clear
  silhouette, transparent where nothing is.
- Keep the texture as small as it can be (a 256×256 holds a whole pair of wings); big textures cost
  memory on every player's machine.

### Glow

A second PNG named `<id>.glow.png`, **the same size as the texture**, is drawn **fullbright** on top
(it ignores the day/night light): paint only the parts that should shine (eyes, gems, rune lines,
feather tips, flames) and leave the rest transparent. Use the exact same pixels as the main texture
under it, or a brighter version. No special entry needed: the file's presence is enough.

### Slots

Each cosmetic has one slot, and a player wears **one per slot**:

| Slot | For |
|---|---|
| `head` | hats, crowns, ears, halos, helmets, headphones |
| `face` | glasses, masks, mustaches |
| `back` | wings, backpacks, capes-that-aren't-capes, tails |
| `shoulders` | pets and small things on a shoulder |
| `arms` | gauntlets, bracers, sleeves (follow the arms) |
| `body` | scarves, belts, necklaces, sashes |

### Idle motion

One looping animation of your bones, `cosmetics/<id>.animation.json` (Bedrock animation file). Rotation
and position keyframes; **add `"lerp_mode": "catmullrom"` to a keyframe for smooth curves** (the kit does
this by default); expressions count as 0. Keep it gentle, 2–4 seconds, and make the two wings move a
little out of step, not mirrored perfectly. `step` holds a value until the next keyframe.

```json
"wing_left": { "rotation": {
  "0":   { "post": [0, -6,  0], "lerp_mode": "catmullrom" },
  "0.8": { "post": [0, -10, -9], "lerp_mode": "catmullrom" },
  "3.2": { "post": [0, -6,  0], "lerp_mode": "catmullrom" } } }
```

### Listing it

```json
{ "id": "angel_wings", "name": "Angel Wings", "slot": "back" }
```

in `cosmetics.json`, with `cosmetics/angel_wings.geo.json`, `cosmetics/angel_wings.png`, optionally
`cosmetics/angel_wings.glow.png` and `cosmetics/angel_wings.animation.json`. The kit writes all four with
the right names.

### Checklist before handing one over

1. `python tools/preview_cosmetic.py <folder> <id>` and **look at it** from all three views next to the
   player. If it looks like a placeholder, it is one.
2. Nothing pokes into the head or body; wings clear the back (z ≥ 2.2); no gaps showing the player
   between feathers unless intended.
3. It reads at small size (the shop shows it a few hundred pixels tall): a clear silhouette first,
   detail second, **one focal point** (a gem, glowing eyes, a feather tip).
4. One theme and one accent color (frost, aurora, ember, forest). No rainbow mixes.
5. Living things move (wings, tails, ears, pets); the motion is smooth and the loop closes.
6. `cargo test -p arctic-cosmetics` passes (it checks every file, including that rectangles lie inside
   the texture and the PNG matches the declared size).

### Quality bar (what to make)

For each cosmetic ask: *would this look at home next to the best cosmetics of the big clients?*

- Wings: 40–100 painted feather cards in 3 rows, fanned, with glowing tips, a slow flap.
- Auras and halos: a large flat ring or disc card facing up or the camera, fine line art, glowing,
  slowly turning (animate a rotation of its bone).
- Headwear: 15–60 pieces; a stepped silhouette, a painted band, one gem that glows.
- Back items and tails: sculpted from several tilted cards or boxes with painted detail, never one
  slab.
- Small shoulder pets: 20–50 cubes with a face that has eyes and a little idle motion.

### Sculpted (mesh) cosmetics

For shapes that boxes and flat cards cannot make (a curved feather, a shell, a horn, a tapering wing profile)
a cosmetic can be a **mesh**: a `.glb` file (glTF 2.0 binary, a restricted subset) that holds triangles,
materials, textures and an idle animation in one file. Cuboid cosmetics keep working exactly as before.
Meshes are for premium pieces: the quality comes from the **shape** (real volume, a continuous silhouette) and the
paint, so budget triangles with care, because every player on screen draws them.

**Files.** `cosmetics/<id>.glb`, plus the usual entry in `cosmetics.json` (`id`, `name`, `slot`). Nothing else is
needed: textures and the animation are inside the `.glb`.

- *Mesh only* (no `<id>.geo.json` / `<id>.png` next to it): older clients that cannot draw meshes **do not see this
  cosmetic at all** (it is listed apart from the cuboid ones, so nothing else in their catalog changes, and nothing
  unrelated takes its place).
- *With a cuboid version* (a `<id>.geo.json` and `<id>.png` as usual): clients that can draw meshes use the mesh,
  the others draw the cuboid version. Use this when the item should be visible everywhere.

**Space.** Positions are in **model pixels** (16 = one block), the same space as the cuboid models: the player stands
on y = 0, y is up, the player faces **-z** (so the back is +z), and **+x is the player's left**. A glTF viewer shows
this file at 16 times the size of a block; that is expected. Place the vertices for the player **at rest**. A **root
node** named `head`, `body`, `rightArm`, `leftArm`, `rightLeg` or `leftLeg` follows that part of the player (any other
name follows the body), through walking, sneaking, swimming and every other pose, moving around the part's rest pivot
(the table above). Give a wing its own child node whose **translation is the shoulder pivot** and whose vertices are
relative to it, so the animation turns it around the shoulder.

**What is supported** (anything else is refused with a message naming the file):

| Feature | Rules |
|---|---|
| Nodes | translation, rotation (quaternion), scale; no `matrix`; at most 32 nodes in one scene tree |
| Geometry | triangle lists; `POSITION`; optional `NORMAL`, `TEXCOORD_0`, `COLOR_0`; indices 8/16/32-bit; no skins, morph targets, sparse accessors |
| Normals | none given: flat shading. Given: smooth shading |
| Materials | `baseColorFactor`, `baseColorTexture`, `emissiveTexture`, `alphaMode` (`OPAQUE`, `MASK`, `BLEND`), `doubleSided`. Metalness, roughness, normal maps are ignored: lighting is Minecraft's |
| Textures | PNG, embedded in the `.glb`, up to 1024 x 1024, at most 6 |
| Animation | one looping animation of node translation, rotation and scale; `LINEAR` or `STEP`; up to 10 s |
| Limits | file 1 MiB; 16 mesh primitives; 6 materials; **24,000 triangles** (hard cap) |

**Lighting.** Surfaces are lit on the side the camera sees (a mesh draws the same whichever way its triangles wind), with
Minecraft's light for where the player stands. Vertex colors and `baseColorFactor` multiply the texture. An
`emissiveTexture` is drawn fullbright on top (accents only; the form must read without it). There is no bloom, no
shadow cast by the item, and no metal/roughness shading.

**Sheen (travelling light).** A mesh can carry a soft band of light that sweeps across its surface once per
period, in the game and in the launcher alike (off when a viewer freezes animations). It lives in the glTF
scene, `scenes[0].extras.arctic.sheen`:

```json
{ "period": 3.0, "tint": [0.63, 0.96, 1.0], "strength": 0.4, "width": 0.065,
  "axis": [0, -1, 0], "origin": -19.4, "length": 7.2,
  "skew": { "axis": [1, 0, 0], "amount": 0.018, "origin": 0, "abs": true } }
```

A point's place in the sweep is `(axis · p - origin) / length` plus `skew.amount * (skew.axis · p - skew.origin)`
(its absolute value with `abs`), with `p` in the file's own model pixels. The band is brightest where that place
equals the loop's progress (0 to 1 over `period`, 0.5 to 10 seconds), is `width` wide (a fraction of the sweep),
fades in and out at the ends of the loop, and is stronger on surfaces facing the camera. `strength` is 0 to 1.
Everything but `period`, `tint` and `strength` is optional. `tools/make_prototype_meshes.py` shows two examples.

**Budgets.** The hard cap is 24,000 triangles; aim for **2,000 to 8,000** for a wing pair or a shoulder pet, **under
1,500** for a small piece. Keep textures at 256 to 512 pixels unless the detail shows; use one material and one texture
sheet where you can.

**Animation.** Author a *rest pose* (the file with the animation off): that is what is drawn when a viewer turns on
**Freeze animated capes and cosmetics**. The animation must loop without a snap (the last key equals the first), move
smoothly, and **never move the item away from its anchor**: turn a wing around its shoulder, bob a pet a pixel or two.

**Checklist.** `cargo run -p arctic-mesh --example check -- <file.glb>` says what the server would say about the file.
Look at it from the back, front, both three-quarter views and both sides next to a player (the launcher's Cosmetics tab
draws it exactly like the game does); check the roots sit on the body, nothing pokes through the player, and the
silhouette reads with the glow off. `cargo test -p arctic-cosmetics` loads every file in `assets/`.

**Tools.** `tools/make_mesh_pilots.py` shows how a plain `.glb` becomes a cosmetic (splitting a wing pair into two
nodes, adding an idle animation); the two pilots, `helios_wings` (back) and `aurora_fox` (shoulders), were made with it.

## Emotes

Whole-player animations played from the emote wheel. **Use the kit**: `assets/tools/cosmetic_kit.py`
has `Animation`, and `assets/tools/example_emote_wave.py` is the quality bar (copy it).

An emote animates the player's own bones: `head`, `body`, `rightArm`, `leftArm`, `rightLeg`,
`leftLeg` (pivots as in the table above). **Rotations (degrees) replace the walking pose** of the
bones you animate; **positions (pixels) are added** to it. Animate every bone that should take part;
the ones you leave out keep walking/idle.

1. One animation, up to **15 seconds**. `loop: true` for emotes that last until the player moves (a
   dance), `false` for a gesture (a wave, a bow). Moving always stops an emote.
2. Export as `emotes/<id>.animation.json` and add `{ "id": "salute", "name": "Salute" }` to `emotes.json`.

### Arm and body conventions

- The right arm lifts outward and up with **`z` positive** (about 150–160 is straight up beside the
  head); the left arm lifts with **`z` negative**. `x` swings an arm forward (negative) or back
  (positive); `y` twists it.
- `head`: `x` negative looks up, positive down; `y` turns left/right; `z` tilts the ear to a shoulder.
- `body`: `x` leans (positive forward), `y` twists the torso, `z` leans sideways. `body` **position**
  (a pixel or two up or down) sells weight, bounce and breathing.
- Legs: `x` swings forward/back; for dances and shuffles use a little `body` position + leg `x`.

### What makes an emote good (the bar)

The current emotes are 3 to 8 poses with straight lines between them; they feel robotic. A good one:

- has **a key pose every 0.15–0.3 s** (a 2.5 s emote has 10–16 keyframes per moving bone) and uses
  **`"lerp_mode": "catmullrom"`** (the kit does) so motion flows through the poses instead of
  moving in straight segments;
- has **anticipation** (a small move the opposite way before the main move) and **follow-through**
  (the head and torso settle a beat after the arm stops);
- uses **the whole body**: head looks at the audience, torso leans and twists, weight shifts (body
  position), the free arm and the legs move a little. An emote where only one arm moves is a wave
  gif, not an emote;
- **hits and holds**: the main pose is held for 0.2–0.4 s with a slight drift, not a single frame;
- starts and ends at the **rest pose** (`0,0,0`) for gestures, and ends where it starts for loops, so
  there is no snap;
- has a clear idea in one sentence ("a proud bow with a hand over the heart", "a shivering hug").

Aim for **20–30 emotes** in a mix: greetings (wave, salute, bow), reactions (laugh, facepalm, shrug,
cheer, cry, shiver), dances (4–6 loops of 2–4 s), poses (flex, thinking, sit-like, victory), and a few
winter ones (snowball throw, warm hands, snow angel arms).

## File formats in short

Geometry (`cosmetics/<id>.geo.json`):

```json
{
  "format_version": "1.12.0",
  "minecraft:geometry": [{
    "description": { "identifier": "geometry.ice_crown", "texture_width": 64, "texture_height": 32 },
    "bones": [{
      "name": "head",
      "pivot": [0, 24, 0],
      "cubes": [
        { "origin": [-4, 32, -5], "size": [8, 2, 1], "uv": [0, 0] },
        { "origin": [-4, 34, -5], "size": [8, 4, 0.25],
          "uv": { "north": { "uv": [0, 16], "uv_size": [32, 16] },
                  "south": { "uv": [32, 16], "uv_size": [-32, 16] } } }
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
