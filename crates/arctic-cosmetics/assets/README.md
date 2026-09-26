# Cosmetics server content

Everything here is loaded and checked when the server starts. A broken file
stops the server with a message naming it, so mistakes never reach players.

## Capes

`catalog.json` lists preset capes; each has `capes/<id>.png` (64×32, or HD
up to 512×256; stack up to 8 frames vertically to animate).

## 3D cosmetics (Blockbench)

1. In Blockbench, create a **Bedrock Entity** model. Start from the player
   template (File → New → Bedrock Entity, then add the player's bones) or
   make your own bones.
2. Name each **root bone** after the player part it follows: `head`,
   `body`, `rightArm`, `leftArm`, `rightLeg` or `leftLeg` (anything else
   follows the body). Put your cubes in those bones or in child bones.
   Use the player template's pivots (head and body `0,24,0`) so the item
   lines up with the player.
3. Use **box UV** (the default). Per-face UV isn't supported.
4. Paint the texture; its sides must be powers of two, up to 512.
5. Export: File → Export → Bedrock Geometry, and save the texture.
6. Optional idle motion (wings flapping, a tail swaying): Animate tab,
   make one looping animation of your bones, File → Export → Bedrock
   Animations. Rotation and position keyframes are used, blended linearly
   (expressions count as 0).
7. Put the files here and add an entry to `cosmetics.json`:

```
cosmetics/<id>.geo.json
cosmetics/<id>.png
cosmetics/<id>.animation.json   (optional)
```

```json
{ "id": "frost_wings", "name": "Frost Wings", "slot": "back" }
```

`id`: 1–32 of `a-z 0-9 _ -`. `slot`: `head`, `face`, `back`, `body` or
`shoulders`; a player wears one item per slot.

Limits: 64 bones, 256 cubes, coordinates within ±256, files up to 256 KB.

## Emotes (Blockbench)

1. Open the player template as a Bedrock Entity (bone names `head`, `body`,
   `rightArm`, `leftArm`, `rightLeg`, `leftLeg`).
2. Animate tab: one animation, up to 15 seconds. Rotations replace the
   walking pose of the bones you animate; positions add to them.
   Set it to loop for emotes that last until the player moves.
3. File → Export → Bedrock Animations as `emotes/<id>.animation.json`, and
   add `{ "id": "wave", "name": "Wave" }` to `emotes.json`.

Players open the emote wheel with **B** (changeable in the Arctic menu,
Looks → Emotes). Moving stops an emote.
