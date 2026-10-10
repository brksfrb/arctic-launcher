# Request: play the reviewed expressive emotes in Arctic

**Status (2026-10-10): done.** All 17 studies are live emotes (new ids, the old wave/bow/clap/facepalm
kept). `crates/arctic-cosmetics/assets/tools/import_emote_specs.py` converts a study into a game emote;
the client's expressive rig (`mod/core/.../looks/Rig.java`, `SkinLimbs.java`) plays root motion, a
bending torso, split chest/pelvis, elbows and knees on every version from 1.8.9 to 26.3. The server
only lists these emotes to clients that ask for the rig (`/v1/cosmetics?rig=2`). The wheel has pages.
Not done: an intro/loop/outro sequence (Twerk plays its 3.5 s once) and reduced-motion handling.

The [17-emote review pack](emote-art-pack/README.md) has authored motion curves and angle-checked videos. It is not ready to copy into `assets/emotes/`: its `motion-spec.json` files are design data, not Bedrock `.animation.json` files, and the preview MP4s are not game assets. Keep the prototypes out of the live catalog until the motions are retargeted and checked in-game.

## Current gap

`mod/core/.../looks/Animation.java` already parses per-bone rotation and position tracks with linear, Catmull-Rom, and step interpolation. The Fabric and legacy `PlayerModelEmoteMixin` paths currently apply those tracks to six independent vanilla player parts: head, body, two arms, and two legs. That is enough for simple gestures but does not reproduce several reviewed motions: parented chest/head motion, a separate pelvis, bent elbows and knees, grounded feet during a squat, or a render-only whole-player turn and lift. Applying only the six tracks would distort the poses and lose the contact points.

## Requested support

1. Define an optional expressive player rig for selected emotes: root, pelvis, chest, head, upper/lower arms, and upper/lower legs. Preserve Minecraft's block proportions and correctly map both base skin and outer layers across the extra segments. Standard six-part animations must keep working.
2. Support authored parent-child transforms and render-only root yaw and position. Root motion must not move the gameplay entity, hitbox, or camera. The head and arms should inherit torso movement so hand-to-head and hand-to-chest contacts stay aligned. Support planted feet or equivalent authored foot targets for crouches and seated poses.
3. Support one-shot clips with a deliberate return to idle, plus an optional intro/loop/outro sequence for repeating emotes. Twerk's preview contains a short repeating beat inside the longer review video; treating the whole review video as one seamless loop would visibly turn the player around on every repetition. Preserve sharp impacts and short holds where authored instead of smoothing every keyframe.
4. Sample the same clip time locally and for other Arctic players, with an immediate, clean interruption when the player starts moving or the emote is stopped. Keep the current server/presence safeguards. Offer sensible reduced-motion behavior for looping emotes.
5. Retarget each approved prototype into a versioned runtime asset with validated duration, joint names, key count, finite transforms, bounded offsets, and an idle/rest pose. Avoid shipping MP4s as runtime content. Verify on supported Minecraft versions and classic/slim player models from front, back, three-quarter, and side views before adding catalog entries.

The in-game emote wheel currently lays all entries around one circle. If most of this pack is added, provide favorites or paging so the wheel remains usable. Keep these emotes free, consistent with the content pack decision.
