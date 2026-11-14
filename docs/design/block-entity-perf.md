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

## Not done, and why

- **Closed chests baked into the section mesh** (only the animated renderer while the lid moves): the
  chest texture is an entity texture, not in the block atlas, so this means stitching every chest texture
  (resource packs, Christmas) into the atlas and rebuilding sections when lids open — per version family.
  Large and easy to get visibly wrong; left for its own change.
- **Occlusion culling** (chests behind walls): ray tests are not conservative (a chest seen through a
  one-pixel gap can vanish) and async ones lag a frame (flicker). The game's section visibility graph
  already removes whole hidden sections; EntityCulling (in the Performance set on modern Fabric and
  Forge 1.8.9) does ray-based culling for those who want it.
- **Polarium** itself has no block entity code and lives in its own repository; this change is in the
  Arctic Client, which runs on every target (Polarium only on 1.8.9 and 26.x).
