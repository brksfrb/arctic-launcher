# Shoulder pets: art direction and review

The first Shadow Pup, Ember Hatchling and Cloud Wisp meshes failed visual review. The replacement pup and wisp use textured meshes. The dragon uses its earlier vertex-painted palette again: the textured export suppressed its hand-painted color patches. Its lower jaw now opens before one short breath in a 4.8-second loop and closes after five emissive flame tongues travel forward and upward. All three pets have a short eyelid blink baked into their GLB idle animation; see the [open/closed comparison](blink-check.png). The three replacements are listed in `cosmetics.json`; reproducible sources are in `source/`.

The model shape must establish the animal before small details are added. Paint a restrained pixel texture with an intentional light-to-dark ramp on each material; a single flat color per face is insufficient. Keep the texture density consistent across pieces. Model and paint in Blockbench or an equivalent voxel-art tool, and export an animated GLB for Arctic. Do not substitute a high-quality render for a weak model.

| Pet | Required silhouette and face | Motion |
| --- | --- | --- |
| Dark dragon | Horizontal flying posture; broad head with a long, unmistakable reptile muzzle and hinged lower jaw; swept horns and layered back plates; four small tucked claws; folded but dimensional wing membranes; a long tapering tail. Keep the flame compact and clearly separate from the mouth. | Body hover, independent wing flex, tail follow-through, jaw opening before a discrete traveling fire breath, occasional blink. |
| Dark pup | Low, chunky seated body; wide head, short deep muzzle, outward/flopped ears, separated paws and a long raised tail. Small white eyes, subtle teal accents and a tiny tongue; avoid a rabbit or humanoid look. | Faster tail wag, small head response, occasional blink. |
| White floating spirit | Simple square body, two short hanging legs, small dark eyes and mouth, pale pink cheeks. Keep the face legible and the body bright from every direction. Avoid decorative horns, crowns or extra tendrils. | Gentle hover, short leg sway, occasional blink. |

The [rotatable 3D viewer](orbit.html) displays the actual GLBs and their idle loops; the [five-angle sheets](index.html) check the attachment silhouette. Before a release, inspect the pets on a moving player in game, including first/last frame seams, equipment overlap, lighting and multiplayer views.

The models under `crates/arctic-cosmetics/assets/cosmetics/` are the selected GLBs. Run `python source/build_v2.py` and `python source/build_textured.py`; copy `source/ember_hatchling.glb` for the dragon, and `source/shadow_pup-textured.glb` and `source/cloud_wisp-textured.glb` for the others. Run `python source/make_data.py` to refresh the embedded 3D viewer assets.
