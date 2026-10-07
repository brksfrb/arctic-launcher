# Request: sculpted 3D cosmetics in Arctic

## Why

Arctic's current cosmetic path loads Bedrock-style geometry made from bones and cubes. Per-face textures, thin cards, idle animation, and fullbright glow are useful, but they cannot reproduce a sculpted feather, curved shell, tapered horn, or continuous wing profile without stacking many boxes or using a flat painted plane. That is the gap seen in the wing and shoulder prototypes: a back view can look convincing while the side reveals a card, excess depth, or a misplaced attachment. More saturation, bloom, or motion alone will not fix the shape.

This document requests a new capability for future cosmetics. It does not change the live catalog, existing models, or game behavior.

## Proposed outcome

1. Keep the existing `.geo.json` cuboid cosmetics working unchanged. Add an optional, versioned mesh format for selected premium-quality items. A constrained GLB/glTF 2.0 subset is a reasonable starting point because the review prototypes already use GLB; finalize the format after checking the target Minecraft renderers. Do not accept arbitrary glTF extensions or remote resources.
2. Support textured triangles with proper normals, a small number of materials, alpha cutouts where needed, and optional vertex colors. Use predictable color-space handling and Minecraft-compatible lighting so the same art does not turn dark or flat in game. An optional emissive mask can use the existing fullbright behavior; bloom should be a separate, bounded effect if supported, not a requirement for a readable model.
3. Define player-space units, handedness, facing direction, attachment slots, bind pose, and root pivots precisely. Mesh cosmetics must follow the appropriate body part through walking, sneaking, swimming, and other player poses. Provide a preview mannequin with the same attachment transform as the in-game renderer.
4. Support a modest skeletal idle animation for parts such as wings and tails, with an authored rest pose when motion is disabled. Animation must be smooth, loop cleanly, and avoid moving the whole item away from its anchor. Respect a viewer's reduced-motion preference.
5. Render both front and back of a deliberately two-sided surface, but prefer closed geometry with real thickness for major pieces. Keep depth testing, face culling, transparency ordering, and normal lighting consistent between launcher preview and game. Check equipment, capes, and other worn cosmetics for clipping.
6. Add a safe asset pipeline: validate file type, byte size, vertex/triangle/bone/material/texture counts, finite coordinates, hierarchy, animation length, and texture dimensions before publishing or allocating GPU resources. Resolve all textures from the content-addressed asset system; reject external URIs, scripts, and unsupported features. Cache decoded meshes and GPU buffers, and release them when no longer used.
7. Add catalog metadata that distinguishes mesh assets from existing cuboid geometry without changing cosmetic IDs, slots, equip rules, or existing clients' parsing. Define an explicit fallback for clients that cannot render meshes, such as a paired cuboid version or hiding only that cosmetic; never substitute an unrelated item.

The [front-visible cosmetic studies](cosmetic-art-pack/README.md) add an arm-slot gauntlet pair and a head-slot visor to the review set. Their shine effects are viewer-only, so animation and attachment behavior need explicit engine support and in-game checks.

## First implementation slice

Build one back-slot wing pair and one shoulder-slot item through the full path: content validation, catalog serving, download, launcher preview, and both supported game adapters. This checks broad and compact silhouettes instead of optimizing only for wings. Keep the mesh budget measurable on several nearby players before raising limits or importing a larger collection.

## Places to review together

- Server catalog and asset validation: `crates/arctic-cosmetics/src/content.rs` and `crates/arctic-cosmetics/src/routes/content.rs`.
- Launcher catalog loading and 3D preview: `crates/arctic-core/src/cosmetic_models.rs` and `crates/arctic-app/src/ui/skins/`.
- In-game parsing, caching, and rendering: `mod/core/src/main/java/com/arcticlauncher/client/looks/Cosmetics.java` and the Fabric and legacy cosmetic adapters.
- Creator instructions and asset layout: `docs/content-guide.md` and `crates/arctic-cosmetics/assets/README.md` once the format is settled.

## Acceptance checks

- Existing cuboid cosmetics look and animate as before. A client without mesh support handles a mesh item according to the documented fallback.
- The two pilot items render on the player in the launcher and game from back, front, both three-quarter views, and both sides at normal play distance. Their attachment and silhouette remain correct while walking, sneaking, and turning.
- Materials show intentional color separation and readable highlights under daylight, shade, and dark conditions. Emissive details remain accents; the form reads with glow and animation off.
- Side views show real volume where intended, with no detached roots, reversed faces, paper-thin main surfaces, or major clipping through the player.
- Animated parts move smoothly and return to their rest pose when paused or reduced motion is enabled. The loop has no visible snap.
- Malformed or excessive assets are rejected safely. With multiple players wearing the pilot assets, frame rate, memory, upload size, and GPU buffer use stay within documented budgets on the supported clients.
