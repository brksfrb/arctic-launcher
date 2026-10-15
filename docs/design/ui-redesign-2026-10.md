# Four redesigns, designed before built (2026-10-15)

Status: **proposal for discussion**. Nothing here is built yet. Each section says what exists
today, what is wrong with it, the design, how it stays light, and what needs a decision.

All four work on every Minecraft version 1.8.9 to 26.3 and every OS, like everything else.

---

## 1. Optional mods ("Extras")

### Today
- One checkbox, "Boost FPS with performance mods", installs a fixed set (Sodium, Lithium,
  FerriteCore, ImmediatelyFast, EntityCulling, ModernFix, Polarium).
- Everything else means searching Modrinth by hand.

### Design
A curated **Extras** catalog: Settings → *Optional mods*, per instance, shown again in setup.
It is a data file (`assets/extras.json`), not code, so adding a mod is one entry.

Each entry: Modrinth slug, one-line "why you'd want it", category, and flags
(`client_only`, `needs`, `conflicts_with_arctic`). Dependencies are installed by the existing
resolver (Fabric API, Cloth Config, YACL, Kotlin are never shown, they come along).
An entry is hidden when Modrinth has no build for the instance's Minecraft version, so the
list is always honest.

**Categories and picks** (from the Modrinth collection, plus Wavy Capes):

| Category | Picks |
|---|---|
| Performance+ | Dynamic FPS (idle when unfocused), More Culling, Enhanced Block Entities, ScalableLux (lighting), FastQuit (instant world exit), Model Gap Fix, Sodium Extra + Reese's Sodium Options (settings), C2ME (chunk loading, singleplayer; flagged "advanced") |
| Looks | Continuity (connected textures), Entity Texture Features + Entity Model Features (packs that need them), 3D Skin Layers, Wavy Capes, Falling Leaves, Cool Rain / Particle Rain, Better Clouds, LambDynamicLights, Fabrishot (huge screenshots) |
| Quality of life | Mouse Tweaks, Jade (what you're looking at), Controlling (search keybinds), Tooltip Scroll, Chest Search Bar, Smooth Scrolling, Borderless Fullscreen, Shulker Box Tooltip and AppleSkin (these are the food / shulker items already on our backlog: ship them as Extras instead of rebuilding) |
| Social | ViaFabricPlus (join servers on other versions), Better Ping Display |
| Maps | Xaero's Minimap (for people who prefer it to ours; turns ours off when installed) |

**Left out on purpose**
- Things Arctic already does: Zoomify (Zoom), Gamma Utils (Fullbright), FPS - Display, BetterF3
  (our HUD), Freelook, Dynamic Crosshair, Cape Provider (our capes), Custom Cursor.
- Server-side mods (VMP, Lithostitched, Alternate Current, WorldEdit...).
- Libraries (they arrive as dependencies).
- Mods that fight shaders or Sodium (Blur+, DashLoader, Nvidium without a warning).

**UI**: a popup with category tabs, a switch per mod, an "all recommended" button, and a size
note. Each switch installs/removes one jar through the normal mod index, so nothing is hidden
from the Mods page. CLI: `arctic extras list|add|remove`.

### Open questions
1. Xaero's Minimap and ours both on screen is silly. OK to switch ours off automatically?
2. C2ME and ScalableLux change world generation / lighting code: ship under "Advanced"
   (off by default, with a one-line warning)?

---

## 2. In-game wardrobe (cosmetics menu)

### Today
The in-game cosmetics menu was removed: looks are chosen in the launcher only. The old tile
classes are dead code. Result: you can't change a cape or cosmetic without leaving the game.

### What was wrong with the last one (from the todo list and our notes)
Jittering previews, scroll that fought the character's zoom, a flat list of 78 items, no
sense of "what am I wearing right now".

### Design
One screen, **three columns**, fixed layout (no scrolling pages inside pages):

```
 [ Cosmetics | Capes | Skins | Emotes ]                       search
 +-------------------+-------------------------+----------------+
 |  Slots            |  Items in the slot      |  Preview       |
 |  Head   (Halo)    |  [thumb][thumb][thumb]  |   your player  |
 |  Face   (-)       |  [thumb][thumb][thumb]  |   turns slowly |
 |  Back   (Wings)   |   filter: slot, tag     |   drag to turn |
 |  Shoulder (-)     |                         |  Wearing: ...  |
 |  Pet    (-)       |                         |  [Fit] [Reset] |
 +-------------------+-------------------------+----------------+
```

- **Slots first.** You see what is worn in each slot at a glance; click a slot to filter.
- **Preview = your real player model** drawn by the game's own entity renderer into the
  panel (the inventory-screen technique), so it can never differ from what others see.
  No second renderer, so no jitter; it only animates while the cursor is over it.
- **Hover = try on, click = wear.** The look is published when you leave the menu or after
  1.5 s idle, one request, never per click.
- **Fit** (up/down, forward, mirror) opens inline for the worn item only.
- **Capes**: same grid, animated ones play only on hover. **Skins**: your launcher library.
  **Emotes**: grid with the wheel assignments.
- Robustness: every network step has a visible state (loading / offline / failed + retry);
  the menu works from the on-disk cache when the cosmetics server is unreachable; a failed
  publish keeps the local choice and retries instead of silently dropping it.

**Lightweight**: thumbnails are rendered once per item to a small atlas (kept on disk by
hash), so the grid is textured quads, not 78 live models.

### Open questions
1. Opened from where: Right Shift menu card, plus a key (default `K`)?
2. Should "Skins" let you pick from the launcher library in-game, or stay launcher-only?
   (The library lives on disk; reading it in-game is easy, uploading to Mojang is not.)

---

## 3. Themes and backgrounds (quality + light animation)

### Today
The menu background is drawn procedurally with flat fills and a few gradient bands. That is
why it looks pixelated and banded, and it cannot be art-directed.

### Design: image themes with a tiny animation vocabulary
A theme is a folder: `assets/arctic/themes/<id>/theme.json` plus PNG/WebP layers.

```json
{
  "id": "aurora-night",
  "name": "Aurora night",
  "palette": { "panel": "#0b1320", "accent": "#7dd3fc", "text": "#e6f1ff" },
  "layers": [
    { "image": "sky.webp",     "fit": "cover" },
    { "image": "aurora.webp",  "fit": "cover", "blend": "add",
      "anim": { "type": "drift", "dx": 0.004, "dy": 0.0, "period": 90 } },
    { "image": "aurora.webp",  "blend": "add", "alpha": 0.6,
      "anim": { "type": "pulse", "min": 0.35, "max": 0.9, "period": 14 } },
    { "image": "stars.webp",   "anim": { "type": "twinkle", "period": 6 } },
    { "image": "ridge.webp",   "fit": "bottom", "anim": { "type": "parallax", "amount": 0.01 } }
  ],
  "particles": { "type": "snow", "count": 40, "wind": 0.02 }
}
```

**Animation vocabulary** (five, nothing else): `drift` (slow scroll), `pulse` (alpha
breathing), `twinkle` (alpha noise), `parallax` (follows the mouse by a few pixels),
`sprite` (N-frame sheet at a low fps). Plus one optional particle type `snow` (max 60).

**Why it stays light**
- Textures are uploaded once; per frame it is N textured quads and a few alpha values.
- No shaders needed (works on 1.8.9's fixed pipeline and 26.x alike).
- Animation pauses when the menu is not the foreground, with a `Reduce motion` switch.
- Image budget: 1920x1080 WebP, under 400 KB per layer, 5 layers max per theme.
- The launcher uses the same theme files for its own backdrop, so one set of art serves both.

**Art**: I cannot paint good illustrations; the brief for an image model is in
[`docs/requests/theme-art-pack/README.md`](../requests/theme-art-pack/README.md), with the
exact layer list and file names the renderer expects. Until art arrives, the existing
procedural scenery stays as the fallback theme.

### Open questions
1. Keep the current four menu styles (Arctic, Aurora, Classic, Vanilla) as the *panel* style,
   and make themes only the *backdrop*? (Recommended: two independent choices.)

---

## 4. Resource packs screen (in game)

### Today
- Minecraft's own screen. Arctic's tab lists packs with on/off, order arrows, Apply, and now
  groups. Browsing Modrinth is a second view.

### Problems
- Order and enabled-ness are one list, so a pack in a group can sit far from its neighbours.
- Apply is a separate step with a reload that freezes the game for seconds.
- No preview of what a pack changes, no conflict hint.

### Design
A two-pane screen:

```
 [ Resource packs | Shaders | Browse ]        search   [group v]
 +-----------------------------+-----------------------------+
 |  Available (off)            |  In use (top wins)          |
 |  [icon] Faithful 32x   [+]  |  1 [icon] Fresh Animations  |
 |  [icon] Bare Bones     [+]  |  2 [icon] Faithful 32x  [-] |
 |  grouped, collapsible       |  drag to reorder            |
 +-----------------------------+-----------------------------+
 |  Selected: description, resolution, size, pack format  [Apply] |
```

- **Drag between panes** to enable, drag within "In use" to reorder (arrows stay for
  keyboard / controller).
- **Groups** are collapsible sections in "Available"; "In use" shows the group as a small tag.
- **Pack format warning** when a pack targets another Minecraft version (it loads anyway,
  but textures may be missing).
- **Apply** only reloads when something changed, shows a progress bar, and the reload runs
  with the game paused (no half-applied state).
- Shaders: same layout with one slot instead of a list.

### Open questions
1. OK to replace Minecraft's native resource-pack screen entirely (as the title and
   server screens already are), keeping a "Vanilla screen" button as the fallback?

---

## Order of work (proposed)
1. Extras catalog (data + popup + CLI), since the collection is already analysed.
2. Resource packs screen (builds on the groups already written).
3. Wardrobe (largest; needs the thumbnail atlas).
4. Themes (engine now, art when it arrives).
