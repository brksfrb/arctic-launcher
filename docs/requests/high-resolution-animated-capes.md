# Request: higher-resolution capes and longer, smoother animations

## Why

The current 128×64 and 256×128 capes leave only 20×32 or 40×64 pixels on the visible back. Fine subjects lose their silhouette and look muddy on a player. Animated capes are limited to eight stacked frames at a fixed 8 FPS, so every loop lasts at most one second. Motion such as a traveling water ripple becomes conspicuously repetitive. Increasing the PNG dimensions alone will not fix artwork that was painted at a lower resolution and enlarged afterward.

This is a request for a format and client capability review. No runtime behavior should change merely because this document exists.

The [cape art pack](cape-art-pack/README.md) contains ten finished visual studies, including eight animated capes. Each animated design includes an explicitly painted still fallback as well as its animated atlas, so the animation choice can be reviewed before implementation.

## Current behavior

- One cape frame uses the standard 2:1 Minecraft cape layout; frames are stacked vertically in one PNG.
- The service, launcher, and game client currently allow 64×32 through 512×256 per frame, at most eight frames.
- Animation playback is fixed at 8 FPS. The client splits the strip into separate textures.
- The service caps uploaded PNGs at 256 KiB. Other request and download limits also need review before accepting larger artwork.

## Proposed outcome

1. Accept capes authored at 1024×512 per frame, preserving the same UV layout and all existing capes.
2. Support longer loops with more than eight frames and a declared playback rate, with 8 FPS as the default for existing assets. A target of 12–16 FPS and roughly 2–4 seconds per loop would make a traveling ripple visibly smoother.
3. Keep the final asset practical to decode and render. If vertically stacked PNGs remain the format, bound both frame count and total strip height. For example, an 8192-pixel strip permits 32 frames at 512×256 or 16 frames at 1024×512. The two new 24-frame 1024×512 review strips are 12,288 pixels tall and therefore need either a higher bounded limit or another frame container. If more high-resolution frames are wanted, use separate frame files or another bounded container rather than an unbounded tall PNG.
4. Make the launcher gallery and in-game previews play the same timing and frames that other players see.
5. Require art painted at the final back-panel resolution (80×128 at 512 width; 160×256 at 1024 width). Enlarging a 40×64 motif should not count as a high-resolution asset.
6. Give each animated cape an **Animate** toggle in its gallery and equipped-cape controls. Turning it off shows that cape's designated still fallback; turning it on plays its frames. Save this wearer choice **per cape ID**, so changing away and back remembers it. Publish the selected animated or still texture hash with the look, allowing older clients to render the chosen state even if they do not understand the toggle. Keep each cape as one gallery item rather than duplicating it into animated and still entries. Existing capes default to animated so prior choices keep their behavior.
7. Also offer a viewer-side reduced-motion setting that freezes animated capes locally, including capes worn by other players. That preference must not change the wearer's published choice. Static capes need no animation control.

## Places to review together

- Service validation and upload limits: `crates/arctic-cosmetics/src/images.rs` and `routes.rs`.
- Launcher cape validation, download limits, and preview texture loading: `crates/arctic-core/src/cosmetics.rs` and `crates/arctic-app/src/ui/skins/`.
- In-game validation, playback timing, HTTP limit, and frame splitting: `mod/core/src/main/java/com/arcticlauncher/client/looks/Looks.java`, `Http.java`, and the Fabric and legacy platform adapters.
- Catalog metadata, import text, and `docs/content-guide.md`, so creators know the supported dimensions, duration, timing, alpha layout, and byte limits.
- Look/presence metadata and settings for the per-cape animation choice, with a designated still texture for each animated cape. The published cape hash should resolve to the wearer's chosen state for older clients too.

## Acceptance checks

- Existing still and eight-frame capes render exactly as before.
- A 1024×512 still cape and a longer animated cape import, upload, download, preview, equip, and render in both supported game adapters.
- Frame timing and last-to-first motion agree between launcher preview and game. A rightward ripple continues rightward across the loop instead of reversing or snapping.
- Oversized, malformed, and excessive-frame uploads are rejected before expensive decoding; memory and GPU use are measured with multiple nearby players wearing animated capes.
- Review assets are inspected on a character from the back, three-quarter, front, and side at normal game zoom. Their subject should remain recognizable there.
- Switching **Animate** off freezes the selected cape consistently in the launcher, on the wearer, and for other players; switching it on restores playback. The choice survives a restart and changing away from and back to that cape. A viewer's reduced-motion setting freezes remote capes only for that viewer.
