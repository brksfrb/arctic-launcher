# Request: play the reviewed expressive emotes in Arctic

**Status (2026-10-12): implemented; full visual verification remains.** All 17 studies are live emotes (new ids, the old wave/bow/clap/facepalm
kept). `crates/arctic-cosmetics/assets/tools/import_emote_specs.py` converts a study into a game emote;
the client's expressive rig (`mod/core/.../looks/Rig.java`, `SkinLimbs.java`) plays root motion, a
bending torso, split chest/pelvis, elbows and knees on every version from 1.8.9 to 26.3. The server
only lists these emotes to clients that ask for the rig (`/v1/cosmetics?rig=2`). The wheel has pages.
Not done: an intro/loop/outro sequence (Twerk plays its 3.5 s once) and reduced-motion handling.

The [17-emote review pack](emote-art-pack/README.md) contains the source motion curves and videos. The runtime assets have already been retargeted into `assets/emotes/`; source `motion-spec.json` files and MP4s are not game assets. The first in-game Minecraft 26.3 pose audit captured all 17 from the front and back at three moments each, with no missing animation. That proves loading and representative poses, while continuous motion and the remaining acceptance checks below still require review.

## Remaining quality gap

The expressive rig now supports parented torso motion, a separate chest and pelvis, elbow and knee segments, and render-only root motion. The unanswered question is whether the retargeted timing, body contact, ground contact, and outer skin layers remain convincing in real playback across classic/slim models and supported versions. The 26.3 still-sheet audit is an initial check, not a substitute for that motion review.

## Requested support

1. Define an optional expressive player rig for selected emotes: root, pelvis, chest, head, upper/lower arms, and upper/lower legs. Preserve Minecraft's block proportions and correctly map both base skin and outer layers across the extra segments. Standard six-part animations must keep working.
2. Support authored parent-child transforms and render-only root yaw and position. Root motion must not move the gameplay entity, hitbox, or camera. The head and arms should inherit torso movement so hand-to-head and hand-to-chest contacts stay aligned. Support planted feet or equivalent authored foot targets for crouches and seated poses.
3. Support one-shot clips with a deliberate return to idle, plus an optional intro/loop/outro sequence for repeating emotes. Twerk's preview contains a short repeating beat inside the longer review video; treating the whole review video as one seamless loop would visibly turn the player around on every repetition. Preserve sharp impacts and short holds where authored instead of smoothing every keyframe.
4. Sample the same clip time locally and for other Arctic players, with an immediate, clean interruption when the player starts moving or the emote is stopped. Keep the current server/presence safeguards. Offer sensible reduced-motion behavior for looping emotes.
5. Retarget each approved prototype into a versioned runtime asset with validated duration, joint names, key count, finite transforms, bounded offsets, and an idle/rest pose. Avoid shipping MP4s as runtime content. Verify on supported Minecraft versions and classic/slim player models from front, back, three-quarter, and side views before adding catalog entries.

The wheel now has pages; favorites remain an optional usability improvement. Keep these emotes free, consistent with the content pack decision.
