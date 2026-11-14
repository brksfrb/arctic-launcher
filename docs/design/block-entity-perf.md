# Chests, hoppers and signs: where the frame goes (static profile)

Read from the game's bytecode (javap of the mapped jars Loom caches) for every target family, not from
runs. Numbers marked *bench* come from the tests named next to them.

## What the game does per frame

**1.8.9–1.12.2 (legacy), 1.13.2, 1.14.4** — `WorldRenderer/LevelRenderer.renderEntities` walks every
visible 16³ section (frustum + visibility graph, *per section*) and calls
`BlockEntityRenderDispatcher.renderEntity(be, tickDelta, -1)` for each block entity in it that has a
renderer. Per call, before the renderer: a squared-distance test (64 blocks), `World.getLight` (chunk
lookup + two light arrays), `glMultiTexCoord2f` (lightmap), `color`, a class→renderer map lookup.
Then the renderer:

| Kind | Per frame, per block entity (1.8.9) |
|---|---|
| Chest / trapped chest | depth state, neighbour check, texture bind (map lookup), push/pop matrix, rescale-normal, ~5 translates, scale, rotate, lid angle from `prevLidAngle/lidAngle`, 3 model parts (display list + 2 translates each): **≈22 GL calls** even when the lid is shut |
| Double chest, second half | everything up to the renderer, then an early return (still a light lookup and 2 GL calls) |
| Ender chest | as a chest, its own texture |
| Sign | board model, then for **each of 4 lines**: `Texts.wrapLines` (component walk, width of every character, new lists/components) + `asFormattedString` + `getStringWidth` + `draw` — the layout part alone is ≈1.4 µs per sign *bench* |
| Skull, banner | model + texture (banner: pattern texture cache lookup) |
| Hopper, furnace, barrel, dropper, dispenser, brewing stand | **no renderer**: the section build leaves them out of the list, so 0 calls per frame |

No block entity is tested against the frustum on its own (vanilla 1.8.9–1.12.2, Fabric 1.13–1.14;
Forge 1.8.9 does test `getRenderBoundingBox`). A section that is a sliver on screen draws every chest in
it.

**1.15–1.21.8** — `LevelRenderer.renderLevel` → for each visible section, for each renderable block
entity: destruction-progress lookup, pose push/translate, `BlockEntityRenderDispatcher.render` →
distance test, light lookup, renderer → vertices into the buffer (chest: 3 cuboids = 72 vertices, lid
rotation, double-chest neighbour combine = 2 block-state + 1 block-entity lookups). Again only
section-level culling.

**1.21.9–26.3** — `extractVisibleBlockEntities` (LevelRenderer up to 26.1.x, LevelExtractor from 26.2)
→ `tryExtractRenderState`: renderer lookup, `isValid(state)`, distance, **a new render-state object per
block entity per frame**, `extractRenderState` (light, lid openness, neighbour combine), later `submit`
of the model. Section-level culling only.

**Ticking (client).** 1.8.9–1.12.2: hoppers, furnaces and chests are in the client's tickable list.
Hopper `update()` returns at once on the client (`world.isClient`): per hopper per tick one loaded-chunk
check and a virtual call, ≈30 µs per tick for 300 hoppers — not worth touching. Chest `update()` moves
the lid angle (a few float ops). 1.17+: hoppers have no client ticker at all; chests only animate the
lid. So hoppers cost the client **nothing per frame** on any version; what they cost is server work
(single-player: the integrated server) and section rebuilds when comparators/redstone next to them
change state — neither is block entity rendering.

## Where it adds up (storage base, *bench*)

`SceneBench` (mod/core tests): 500 chests, 300 hoppers, 125 signs, 20 ender chests, 40 shulker boxes,
24 furnaces, 24 barrels; 400 camera views inside the base; the game's section frustum stood in for by a
plain six-plane frustum (visibility graph left out, so both columns are upper bounds):

| Renderer calls per frame | Game | Block entity culling |
|---|---|---|
| average | 336.8 | 210.0 (−38%) |
| worst view | 685 | 641 |
| with a 32-block draw distance | | 159.3 |
| chests / signs / ender / shulker | 244.1 / 62.4 / 10.2 / 20.2 | 153.4 / 38.4 / 5.9 / 12.3 |
| hoppers, furnaces, barrels | 0 | 0 |

On 1.8.9 that is ≈5.4k → ≈3.4k GL calls a frame for the chests alone (22 each). The test itself costs
≈14 ns per block entity.

Sign text on 1.8.9 (`SignLayoutBenchTest`, the game's own `Texts.wrapLines` on a headless font):
125 signs × 4 lines, 176 µs per frame → 4 µs with the kept layout.

## What changed

- **Block entity culling** (Mods → Block Entity Culling, on by default): each chest, trapped chest,
  ender chest, shulker box, sign, hanging sign, skull, banner, bed, bell, lectern, campfire and decorated
  pot is tested against the game's own frustum with its block grown by one block on every side (double
  chests, open lids, shulker lids, standing/wall banners all fit). Only those exact classes: modded
  subclasses, beacons, end gateways, structure blocks and spawners are left to the game. Hooked at the
  head of the dispatcher's per-block-entity method, before the light lookup and render-state creation.
  - legacy (1.8.9–1.12.2, Forge 1.8.9 too): the `CameraView` that `renderEntities` is given.
  - 1.13.2, 1.14.4: the `Culler` that `renderEntities` is given.
  - 1.15+: the first `Frustum` the frame's entities are tested with (`EntityRenderDispatcher.shouldRender`,
    always before block entities), only between the start and end of the world pass; outside it
    (previews, GUI) nothing is culled. Off when Iris/Oculus/OptiFabric or Immersive Portals is loaded
    (they draw block entities again with a different view).
- **Draw distance** for those kinds, 16–64 blocks; 64 (default) is the game's own and changes nothing.
- **Sign layout kept** (1.8.9–1.12.2 Fabric): the wrapped line and its formatted text are kept per sign
  while the sign holds the same line objects and the font measures the same (probe string width), so
  edits, server updates, resource reloads and the smooth font all lay it out again.

## Chests on the GPU (26.2)

Culling still leaves every chest in view going through `tryExtractRenderState` → a new render state →
`submit` → 72 vertices built on the CPU and uploaded, every frame. A storage hall with tens of thousands
of chests in view is hundreds of megabytes of vertices a frame. Polarium draws crowds of entities another
way, and the chests now go the same way.

**How Polarium draws entities** (its own repository; 26.2 and 26.3 have the GPU path, 26.1.2 only the CPU
side, 1.8.9 none of this: there it batches terrain regions and caches clouds):

- A model's shape is uploaded once, in each part's own space (`ModelMesh`, `ItemMesh`).
- Per entity only a small block of RGBA32F texels goes up each frame (pose rows or a skeleton, light,
  overlay, colour, where its skin sits in the skin atlas), into one texel buffer per frame; three buffers
  are used in turn, each guarded by a fence.
- For each of the game's entity pipelines a twin is made that differs only in its vertex shader and vertex
  format (`InstancedPipelines`); the vertex shader is the game's own, as loaded, with its inputs turned
  into variables that Polarium's part fills in per instance (`ShaderSources`). Fragment shader, blending,
  depth, textures, fog and light are the game's, so pixels match.
- One instanced draw per (mesh, render type) per group (`GpuBatches`), issued where the game draws that
  group. Whatever it doesn't know (other mods' layers, odd poses) stays on the game's path; with a shader
  mod or a model mod everything does.

**The chest path** (`ChestInstancing` in the core, `GpuChests` in the 26.2 adapter) is that, plus what
entities can't have: a chest that isn't moving is not looked at again.

1. *Culling first.* The game (or Sodium) still decides which sections' block entities are drawn. Each
   chest reaches `BlockEntityRenderDispatcher.tryExtractRenderState`, where it is taken: no render state,
   no submit. Per chest that is a field read and a mark on its section (≈3 ns).
2. *Shut chests are kept per section.* A section's shut chests are written once into a block of
   instances (32 bytes each: block position from an anchor, facing, light, texture) per model (single,
   left half, right half). The block is written again only when the section changes: a chest placed,
   broken or turned into a double chest (`setRemoved`, `setBlockState`), a lid event, light
   (`ClientChunkCache.onLightUpdate`, `ClientLevel.setSectionDirtyWithNeighbors`; read twice, a frame
   apart, as the light may land a frame late).
3. *One buffer, three draws.* Each frame the blocks of the sections in view are copied into one texel
   buffer and each model is drawn once, instanced, through a twin of the game's chest pipeline
   (`entity_cutout_cull` on the chest atlas). While the same sections are in view and no lid moves, last
   frame's buffer is drawn again: nothing is copied or uploaded. The vertex shader places each vertex
   (facing, block, camera), so positions are exact at any coordinate (whole blocks from the anchor; the
   camera's fraction is subtracted last), and leaves out chests farther than the draw distance, as the
   game does (`shouldRender`, 64 blocks, or the mod's shorter distance).
4. *Moving lids.* The game's block event 1 puts a chest (and the other half of a double chest) on the
   moving path: its instance is written every frame with the lid's eased angle, read exactly as
   `ChestRenderer` reads it, and the shader turns the lid and lock about the hinge. Once shut it goes
   back into its section's block. Same draw, same shader.
5. *Being broken.* A chest with a breaking overlay is left to the game for as long as it has one (the
   game draws the cracks over its own geometry) and left out of the instances.
6. *Textures and light.* The textures are the game's sprites in its chest atlas (resource packs, trapped,
   ender, the four copper states, Christmas: picked as `ChestRenderer.getChestMaterial` picks them); the
   models are the game's baked chest layers; light is the game's packed light for the block (the brighter
   half for a double chest), looked up in the game's lightmap by the game's shader. All of it is taken
   again after a resource reload.

Fallbacks: off with Iris, Oculus, OptiFabric, Immersive Portals, Entity Model/Texture Features, Enhanced
Block Entities, FastChest, Canvas or VulkanMod; off for any chest whose renderer isn't the game's
`ChestRenderer` or whose class isn't exactly the game's (modded chests); and it turns itself off (logged)
if the pipeline doesn't compile or anything throws, after which chests are drawn by the game again. Sodium
is fine: it calls the same dispatcher. Mods → Block Entity Culling → Chests → "Draw chests on the GPU"
(on by default); `-Darctic.gpuChests=false` forces it off, `-Darctic.gpuChests.report=true` logs counts.

### 70,000 chests, 35,000 hoppers (*bench*: `ChestSceneBench`)

A hall with 70,000 chests (35,000 double chests) and 35,000 hoppers, all inside the 64-block draw
distance; 48 views from its middle. Counts are exact; the "vertex work" row is a stand-in (72 vertices
through a matrix into 36-byte vertices) that leaves out the render state, the model walk, the light and
neighbour lookups and the upload, so the game's real cost is higher.

| Per frame | Game (origin/main) | + block entity culling | + chests on the GPU |
|---|---|---|---|
| chest renderer calls, average (worst view) | 29,067 (34,532) | 17,460 (25,702) | 0 |
| render states made | 29,067 | 17,460 | 0 |
| vertices built on the CPU | 2.09 million | 1.26 million | 0 |
| vertex / instance data uploaded | 75.3 MB | 45.3 MB | 0 with the view still; 0.93 MB while turning or while a lid moves |
| chest draw calls | 1 (the game batches them) | 1 | 3 (instanced), whatever the count |
| CPU, vertex work alone (stand-in, a floor) | ≈11 ms | ≈7 ms | – |
| CPU, measured: view still / turning / ten lids moving | – | – | 0.11 / 0.12 / 0.12 ms |
| hoppers | 0 calls, 0 draws | 0 | 0 |

Memory with all 70,000 kept: 2.24 MB of instance blocks and 2.24 MB of frame buffer on the CPU, up
to 10.1 MB on the GPU (three buffers in turn, with headroom), 7.8 KB of models. First sight of all
70,000 (every section written once): 8–15 ms.

One real start (26.2 with Sodium, Fabric API and Polarium, software GL): the shader compiled, 138 chests
of every kind went out in 3 draws, and screenshots of the same view with the option off and on match
(singles in all four facings, double chests, trapped, ender, copper, a chest being broken, chests
removed); the pixels that differ are clouds (seen between the chests) and particles.

**Hoppers** on 26.2: no renderer (never in a section's block entity list), no client ticker
(`HopperBlock.getTicker` returns null on the client), nothing sent per tick; their cooldown never
reaches the client. There is no client work to remove.

## Not done, and why

- **Other versions.** The chest path is 26.2 only (26.3 draws inside the game's own render pass and
  compiles shaders to SPIR-V; 1.21.x and older have other pipelines). They keep the culling above.
- **Shulker boxes, signs, banners, beds on the GPU.** Same method (their own models, atlases and
  animations); not done yet. Barrels, hoppers, furnaces are plain blocks already.
- **Skipping the client's lid tick for shut chests** (70,000 chests are ticked 20 times a second to move
  lids that are shut, ≈50 ns each). Tried and taken out: in the one real start a lid was seen part open
  in both the game's and the GPU drawing, which could not be explained without more runs.
- **Dropping the per-chest visit altogether** (≈3 ns per chest in view): would mean taking chests out
  of the section's block entity list, once for the game and once for Sodium.
- **Occlusion culling** (chests behind walls): ray tests are not conservative (a chest seen through a
  one-pixel gap can vanish) and async ones lag a frame (flicker). The game's section visibility graph
  already removes whole hidden sections; EntityCulling (in the Performance set on modern Fabric and
  Forge 1.8.9) does ray-based culling for those who want it.
- **In Polarium itself.** It lives in its own repository and has no block entity code; the chest path
  is in the Arctic Client (which is what this repository builds) and borrows Polarium's method.
