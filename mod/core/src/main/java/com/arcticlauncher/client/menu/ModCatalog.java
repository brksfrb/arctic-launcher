package com.arcticlauncher.client.menu;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.feature.CombatFx;
import com.arcticlauncher.client.feature.DebugStyle;
import com.arcticlauncher.client.feature.QuickMessages;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.menu.Mod.Category;
import com.arcticlauncher.client.tiers.Tiers;

/** Every card on the Mods screen, for this version of the game. */
final class ModCatalog {
	private static final int[] OUTLINE_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFF000000};
	/** Unset, the pointer takes the menu's accent color. */
	private static final int[] POINTER_COLORS = {0xFF7DD3FC, 0xFFFFFFFF, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9};
	private static final int[] HIT_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF97316, 0xFFE879F9};
	private static final String[] TIMES = {"Day", "Noon", "Sunset", "Night", "Midnight"};
	private static final int[] TIME_TICKS = {1000, 6000, 12500, 13500, 18000};
	private static final String[] WIDTHS = {"Thin", "Normal", "Thick"};
	private static final float[] WIDTH_VALUES = {1f, 2f, 3f};
	/** Motion blur strengths, 1 to 4. */
	private static final String[] BLUR_MODES = {"Turn + move", "Turning", "Moving"};

	private ModCatalog() {}

	/** The Mods screen's filter chips for HUD widgets. */
	static final String[] HUD_GROUPS = {"All", "Performance", "PvP", "World", "Info"};

	/** Where each mod is listed (the cards themselves are declared below by what they do). */
	private static final java.util.Map<String, Category> REGROUP = new java.util.HashMap<String, Category>();
	/** The group of each HUD widget; the rest are "Info". */
	private static final java.util.Map<String, String> WIDGET_GROUP = new java.util.HashMap<String, String>();
	/** Features that came with the latest version: they carry a NEW mark until they are opened or switched. */
	private static final java.util.Set<String> NEW_IDS = new java.util.HashSet<String>();

	private static void group(Category category, String... ids) {
		for (String id : ids) {
			REGROUP.put(id, category);
		}
	}

	private static void widgets(String group, String... ids) {
		for (String id : ids) {
			WIDGET_GROUP.put(id, group);
		}
	}

	static {
		group(Category.COMBAT, "combatfx", "tnttimer", "hitboxes", "durability", "hitcolor");
		group(Category.INVENTORY, "slotlocks", "invsearch", "shulkerpeek");
		group(Category.MOVEMENT, "zoom", "freelook", "sprint", "sneak", "portalscreens", "controller");
		group(Category.TOOLS, "waypoints", "screenshots", "replays", "death", "leave", "reconnect");
		group(Category.PLAYERS, "pingtags", "badgetags", "ownnametag", "ofcapes", "tabping", "tabglow", "tiers");
		widgets("Performance", "fps", "ping", "tps", "frametime", "lows", "cpu", "syscpu", "sysram", "alloc", "gpu", "vram", "memory",
				"entities");
		widgets("PvP", "cps", "keystrokes", "armor", "effects", "arrows", "items", "held", "pearl", "blocking", "falldamage", "target");
		widgets("World", "coords", "direction", "speed", "biome", "light", "chunk", "day", "worldtime", "clock", "date", "compass",
				"minimap", "death", "stopwatch");
		for (String id : new String[] {"combatfx", "slotlocks", "invsearch", "shulkerpeek", "particles", "helditem", "palette",
				"portalscreens", "tnttimer", "ownnametag", "ofcapes", "nohurtcam", "tabping", "badgetags", "hud.cpu", "hud.syscpu", "hud.sysram", "hud.alloc",
				"hud.gpu", "hud.vram", "hud.frametime", "hud.lows", "hud.blocking", "hud.falldamage", "hud.pearl"}) {
			NEW_IDS.add(id);
		}
	}

	/** Has the NEW mark: a feature of the latest version nobody has opened or switched yet. */
	static boolean isNew(String id) {
		ClientConfig c = ArcticClient.config();
		return NEW_IDS.contains(id) && (c.seenMods == null || !c.seenMods.contains(id));
	}

	/** How many NEW features nobody has opened yet. */
	static int newCount() {
		int n = 0;
		for (String id : NEW_IDS) {
			if (isNew(id)) {
				n++;
			}
		}
		return n;
	}

	static void markSeen(String id) {
		ClientConfig c = ArcticClient.config();
		if (NEW_IDS.contains(id) && isNew(id)) {
			if (c.seenMods == null) {
				c.seenMods = new java.util.ArrayList<String>();
			}
			c.seenMods.add(id);
			ArcticClient.saveConfig();
		}
	}

	static List<Mod> all() {
		final ClientConfig c = ArcticClient.config();
		Platform p = ArcticClient.platform();
		List<Mod> out = new ArrayList<Mod>();
		boolean game = p.hasFeatures();
		if (game) {
			gameplay(out, c);
			visuals(out, c, p);
			chat(out, c);
		}
		social(out, c);
		out.add(hudStyle(c));
		for (final HudWidget w : ArcticClient.hud().widgets()) {
			final HudSlot slot = ArcticClient.hud().slot(w);
			Mod m = new Mod("hud." + w.id, w.name, w.description, w.id, Category.HUD, "hud widget" + widgetKeywords(w.id))
					// Where it was placed is kept while it's off.
					.toggle(() -> slot.enabled, on -> slot.enabled = on)
					.settings((host, f) -> {
						if ("compass".equals(w.id)) {
							f.section("Pointer");
							f.swatches("Color", POINTER_COLORS, () -> c.compassPointer, v -> c.compassPointer = v);
						}
						if ("minimap".equals(w.id)) {
							f.section("Map");
							f.choice("Shape", new String[] {"Square", "Round"}, () -> c.minimapRound ? 1 : 0, i -> c.minimapRound = i == 1);
							f.toggle("Turn with me", "The way you face stays up (otherwise north is up)", () -> c.minimapRotate,
									on -> c.minimapRotate = on);
							f.choice("Size", labels(ClientConfig.MINIMAP_SIZES, ""),
									() -> Math.max(0, indexOf(ClientConfig.MINIMAP_SIZES, c.minimapSize)),
									i -> c.minimapSize = ClientConfig.MINIMAP_SIZES[i]);
							f.choice("Shows", labels(ClientConfig.MINIMAP_ZOOMS, " blocks"),
									() -> Math.max(0, indexOf(ClientConfig.MINIMAP_ZOOMS, c.minimapZoom)),
									i -> c.minimapZoom = ClientConfig.MINIMAP_ZOOMS[i]);
							f.toggle("Other players", "Arrows for the players near you", () -> c.minimapPlayers, on -> c.minimapPlayers = on);
							f.toggle("Coordinates", "Your X and Z under the map", () -> c.minimapCoords, on -> c.minimapCoords = on);
						}
						if ("coords".equals(w.id)) {
							f.section("Numbers");
							f.stepper("Decimals", () -> c.coordDecimals, v -> c.coordDecimals = (int) v, 0, 2, 1, "%.0f");
						}
						if ("stopwatch".equals(w.id)) {
							f.section("Key");
							f.key("Stopwatch key", "Starts, stops and clears it", Form.key(() -> c.stopwatchKey, k -> c.stopwatchKey = k));
						}
						HudLookForm.build(host, f, w);
					});
			m.widget = w;
			out.add(m);
		}
		for (Mod m : out) {
			Category moved = REGROUP.get(m.id);
			if (moved != null) {
				m.category = moved;
			}
			if (m.id.startsWith("hud.")) {
				String group = WIDGET_GROUP.get(m.id.substring(4));
				m.group = group == null ? "Info" : group;
			}
		}
		if (!p.hasScreenHooks()) {
			// This version has no hooks for them: better not offered than switched on and silent.
			List<String> missing = java.util.Arrays.asList("slotlocks", "invsearch", "particles", "helditem", "shulkerpeek");
			for (int i = out.size() - 1; i >= 0; i--) {
				if (missing.contains(out.get(i).id)) {
					out.remove(i);
				}
			}
		}
		return out;
	}

	private static void gameplay(List<Mod> out, final ClientConfig c) {
		out.add(new Mod("zoom", "Zoom", "Hold a key to zoom in; scroll to zoom further", "zoom", Category.GAMEPLAY, "optifine spyglass")
				.toggle(() -> c.zoomEnabled, on -> c.zoomEnabled = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Zoom key", "Hold it to zoom; scroll while zooming to go further", Form.key(() -> c.zoomKey, k -> c.zoomKey = k));
					f.section("Animation");
					f.choice("Curve", ClientConfig.ZOOM_CURVES, () -> c.zoomAnimation, i -> c.zoomAnimation = i);
					f.stepper("Duration", () -> c.zoomDurationMs, v -> c.zoomDurationMs = (int) v, 0, 1000, 50, "%.0f ms");
					f.toggle("Smooth scroll zoom", "The wheel eases to the new zoom level instead of jumping",
							() -> c.zoomSmoothScroll, on -> c.zoomSmoothScroll = on);
					f.section("Feel");
					f.toggle("Smooth camera while zoomed", "Like OptiFine's zoom: the view glides instead of snapping",
							() -> c.zoomSmoothCamera, on -> c.zoomSmoothCamera = on);
					f.stepper("Zoom sensitivity", () -> c.zoomSensitivity, v -> c.zoomSensitivity = (int) v, 10, 200, 5, "%.0f%%");
				}));
		out.add(new Mod("emotes", "Emotes", "Hold a key, point at an emote, let go: others see it", "looks", Category.SOCIAL,
				"emote wheel dance wave key rebind")
				.settings((host, f) -> {
					f.section("Key");
					f.key("Emote wheel key", "Hold it, move the mouse to an emote, let go",
							Form.key(() -> c.emoteKey, k -> c.emoteKey = k));
					f.note("Pick your look and emotes in Arctic Launcher → Cosmetics.");
				}));
		out.add(new Mod("freelook", "Freelook", "Hold a key to look around without turning", "freelook", Category.GAMEPLAY, "perspective 360")
				.toggle(() -> c.freelookEnabled, on -> c.freelookEnabled = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Freelook key", "Hold it to look around", Form.key(() -> c.freelookKey, k -> c.freelookKey = k));
					f.note("Some servers (like Hypixel) don't allow it; it stays off there.");
				}));
		out.add(new Mod("portalscreens", "Portal Screens", "Keep the inventory, chat and menus open while you stand in a nether portal", "portalscreens",
				Category.GAMEPLAY, "nether portal gui inventory chat menu close screen")
				.toggle(() -> c.portalScreens, on -> c.portalScreens = on));
		out.add(new Mod("tnttimer", "TNT Timer", "The seconds left on a lit TNT, floating above it", "tnt", Category.GAMEPLAY,
				"tnt explosion fuse countdown timer bomb")
				.toggle(() -> c.tntTimer, on -> c.tntTimer = on));
		out.add(new Mod("sprint", "Toggle Sprint", "Press sprint once to keep sprinting", "sprint", Category.GAMEPLAY, "run")
				.toggle(() -> c.toggleSprint, on -> c.toggleSprint = on));
		out.add(new Mod("sneak", "Toggle Sneak", "Press sneak once to keep sneaking", "sneak", Category.GAMEPLAY, "crouch shift")
				.toggle(() -> c.toggleSneak, on -> c.toggleSneak = on));
		out.add(new Mod("waypoints", "Waypoints", "Mark places and see them in the world", "waypoints", Category.GAMEPLAY, "marker home death")
				.toggle(() -> c.waypointsInWorld, on -> c.waypointsInWorld = on)
				.page(WaypointsTab::new));
		out.add(new Mod("screenshots", "Screenshots", "Copy new screenshots to the clipboard", "camera", Category.GAMEPLAY,
				"clipboard copy f2 picture")
				.toggle(() -> c.copyScreenshots, on -> c.copyScreenshots = on)
				.settings((host, f) -> {
					f.section("Copy");
					f.toggle("Copy automatically", "Every new screenshot goes onto the clipboard", () -> c.copyScreenshots,
							on -> c.copyScreenshots = on);
					f.note("When it's off, the pop-up has a Copy button: open chat or the");
					f.note("inventory to free the mouse, then click it.");
				}));
		// Only where this version can record and play replays.
		if (com.arcticlauncher.client.replay.Replays.canWatch()) {
			out.add(new Mod("replays", "Replays", "Instant clips, start/stop recording and queued videos", "replay", Category.GAMEPLAY,
					"replay record clip video camera recording")
					.page(ReplaysTab::new));
		}
		out.add(new Mod("death", "Death Position", "A pop-up with where you died, and a Death waypoint", "death", Category.GAMEPLAY, "coords")
				.toggle(() -> c.deathNotice, on -> c.deathNotice = on));
		out.add(new Mod("pingtags", "Ping Tags", "Other players' ping in front of their name tag: 48ms | Steve", "tiers", Category.SOCIAL,
				"ping latency nametag lag ms")
				.toggle(() -> c.pingNametags, on -> c.pingNametags = on));
		out.add(new Mod("badgetags", "Arctic Badge", "The snowflake in front of other Arctic players' name tags (1.16 and newer)", "badgetags", Category.SOCIAL,
				"badge snowflake arctic nametag tag above head")
				.toggle(() -> c.badgeNametags, on -> c.badgeNametags = on));
		out.add(new Mod("ownnametag", "Own Name Tag", "See your own name tag above your head in third person: badge, tier and name", "ownnametag", Category.SOCIAL,
				"nametag tag above head self own name third person")
				.toggle(() -> c.ownNametag, on -> c.ownNametag = on));
		out.add(new Mod("ofcapes", "OptiFine Capes", "Show other players' OptiFine capes, without OptiFine (asks OptiFine's cape server for each player's name, as OptiFine does)", "looks", Category.SOCIAL,
				"optifine cape capes cloak")
				.toggle(() -> c.optifineCapes, on -> c.optifineCapes = on));
		out.add(new Mod("dynamicfps", "Dynamic FPS", "A lower frame rate limit while the game is in another window, idle or in a menu (1.14 to 1.21.11 and 1.8.9 to 1.12.2)", "particles", Category.VISUAL,
				"fps limit background unfocused idle afk battery save power performance")
				.toggle(() -> c.dynamicFps, on -> c.dynamicFps = on)
				.settings((host, f) -> {
					f.section("Limits (0 = no limit of that kind)");
					f.stepper("In another window", () -> c.dynFpsUnfocused, v -> c.dynFpsUnfocused = (int) v, 0, 60, 5, "%.0f FPS");
					f.stepper("Idle", () -> c.dynFpsIdle, v -> c.dynFpsIdle = (int) v, 0, 120, 5, "%.0f FPS");
					f.stepper("Idle after", () -> c.dynFpsIdleSeconds, v -> c.dynFpsIdleSeconds = (int) v, 10, 600, 10, "%.0f s");
					f.stepper("In a menu", () -> c.dynFpsMenu, v -> c.dynFpsMenu = (int) v, 0, 240, 10, "%.0f FPS");
					f.note("Idle means nothing moved: not you, not the mouse. The game's own limit still applies; this only lowers it.");
				}));
		out.add(new Mod("nohurtcam", "No Hurt Shake", "The view stops tilting and shaking when you take damage (1.14 to 1.21.11)", "particles", Category.VISUAL,
				"hurt cam camera shake shaking tilt damage bob")
				.toggle(() -> c.noHurtCam, on -> c.noHurtCam = on));
		out.add(new Mod("tabping", "Tab Ping", "The ping in milliseconds in the player list, where the ping bars were", "tabping", Category.SOCIAL,
				"tab list ping latency ms players")
				.toggle(() -> c.tabPing, on -> c.tabPing = on));
		out.add(new Mod("tabglow", "Find Me in Tab", "Your own name in the player list pulses: » Steve «", "tiers", Category.SOCIAL,
				"tab list find myself name highlight glow")
				.toggle(() -> c.tabGlowSelf, on -> c.tabGlowSelf = on));
		out.add(new Mod("hitboxes", "Hitboxes", "Every player's and mob's hitbox as a box, like F3+B in one click", "target",
				Category.GAMEPLAY, "hitbox combat box f3 reach")
				.toggle(() -> c.hitboxes, on -> c.hitboxes = on));
		out.add(new Mod("debugscreen", "Debug Screen", "A colorful F3 screen: labels and values in colors", "debugscreen",
				Category.VISUAL, "f3 debug betterf3 coordinates info")
				.toggle(() -> c.debugStyle, on -> c.debugStyle = on)
				.settings((host, f) -> {
					f.section("Colors");
					f.choice("Labels", DebugStyle.COLORS, () -> c.debugLabelColor, i -> c.debugLabelColor = i);
					f.choice("Values", DebugStyle.COLORS, () -> c.debugValueColor, i -> c.debugValueColor = i);
					f.section("Size");
					f.stepper("Text size", () -> c.debugScale, v -> c.debugScale = (int) v, 50, 300, 10, "%.0f%%");
					f.section("Lines");
					for (int g = 0; g < DebugStyle.GROUPS.length; g++) {
						final int group = g;
						f.toggle(DebugStyle.GROUPS[g].label, "Show these lines on the F3 screen", () -> !DebugStyle.hidden(c, group),
								on -> DebugStyle.setHidden(c, group, !on));
					}
					f.note("Press F3 to see it. A line this does not know stays visible.");
				}));
		out.add(new Mod("saturation", "Saturation Bar", "Your hidden saturation as a golden glow on the hunger icons", "food",
				Category.HUD, "hunger food appleskin saturation")
				.toggle(() -> c.saturationBar, on -> c.saturationBar = on));
		out.add(new Mod("durability", "Low Durability", "A pop-up when armor or your tool is about to break", "durability",
				Category.GAMEPLAY, "armor tool break warning")
				.toggle(() -> c.durabilityWarning, on -> c.durabilityWarning = on));
		out.add(new Mod("combatfx", "Combat Feedback", "A hit marker, hit and kill sounds, and a low-health warning", "combatfx",
				Category.GAMEPLAY, "hitmarker kill sound ding low health warning pvp")
				.toggle(() -> c.hitMarker, on -> c.hitMarker = on)
				.settings((host, f) -> {
					f.section("Hit marker");
					f.toggle("Hit marker", "A small X at your crosshair when you hit; red on a kill", () -> c.hitMarker,
							on -> c.hitMarker = on);
					f.swatches("Color", CombatFx.MARKER_COLORS, () -> c.hitMarkerColor, i -> c.hitMarkerColor = i);
					f.section("Sounds");
					f.choice("Hit sound", CombatFx.SOUNDS, () -> c.hitSound, i -> c.hitSound = i);
					f.choice("Kill sound", CombatFx.SOUNDS, () -> c.killSound, i -> c.killSound = i);
					f.section("Low health");
					f.toggle("Low-health warning", "A red pulse at the screen edges", () -> c.lowHealthWarning,
							on -> c.lowHealthWarning = on);
					f.stepper("Hearts", () -> c.lowHealthHearts, v -> c.lowHealthHearts = (int) v, 1, 9, 1, "%.0f");
					f.toggle("Warning beep", "A soft beep while it pulses", () -> c.lowHealthBeep, on -> c.lowHealthBeep = on);
					f.note("A hit counts when you attack; the server may still ignore it.");
				}));
		out.add(new Mod("slotlocks", "Lock Slots", "Lock inventory slots so their items can't be moved or dropped by accident", "slotlocks",
				Category.GAMEPLAY, "inventory lock protect slot drop hotbar")
				.toggle(() -> c.slotLocks, on -> c.slotLocks = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Lock key", "Hold it and click a slot to lock or unlock it",
							Form.key(() -> c.slotLockKey, k -> c.slotLockKey = k));
					f.note("A locked slot shows a padlock. Its item can't be clicked, shift-clicked,");
					f.note("swapped with a number key or thrown out with the drop key.");
				}));
		out.add(new Mod("invsearch", "Inventory Search", "Ctrl+F in a chest or your inventory: find items by name", "invsearch",
				Category.GAMEPLAY, "chest search find item inventory shulker")
				.toggle(() -> c.containerSearch, on -> c.containerSearch = on)
				.settings((host, f) -> {
					f.section("How it works");
					f.note("Press Ctrl+F in any chest, barrel or your inventory and type.");
					f.note("Matching items get a frame, the others are dimmed.");
					f.note("Enter keeps the search, Escape clears it.");
				}));
		out.add(new Mod("palette", "Command Palette", "Ctrl+K: search mods, servers and friends and open them", "palette",
				Category.CLIENT, "search command ctrl k quick find launcher")
				.toggle(() -> c.commandPalette, on -> c.commandPalette = on)
				.settings((host, f) -> {
					f.section("How it works");
					f.note("Press Ctrl+K anywhere in the game, type, and press Enter.");
					f.note("It finds mods and their settings, saved servers and friends.");
				}));
		out.add(new Mod("particles", "Particle Control", "Show only a share of the particles (fewer = more FPS and a clearer view)", "particles",
				Category.VISUAL, "particles fps performance crit smoke explosion")
				.toggle(() -> c.particleControl, on -> c.particleControl = on)
				.settings((host, f) -> {
					f.section("Amount");
					f.stepper("Show", () -> c.particleAmount, v -> c.particleAmount = (int) v, 0, 100, 10, "%.0f%%");
					f.note("100% is the game's own amount; 0% shows none.");
				}));
		out.add(new Mod("helditem", "Held Item", "Move and resize your hands and the item you hold", "helditem",
				Category.VISUAL, "hand item position size view model offset")
				.toggle(() -> c.heldItem, on -> c.heldItem = on)
				.settings((host, f) -> {
					f.section("Position");
					f.stepper("Right / left", () -> c.heldX, v -> c.heldX = (int) v, -50, 50, 5, "%.0f");
					f.stepper("Up / down", () -> c.heldY, v -> c.heldY = (int) v, -50, 50, 5, "%.0f");
					f.stepper("Away / closer", () -> c.heldZ, v -> c.heldZ = (int) v, -50, 50, 5, "%.0f");
					f.section("Size");
					f.stepper("Size", () -> c.heldScale, v -> c.heldScale = (int) v, 30, 150, 10, "%.0f%%");
					f.note("Look at your hand in the game to see each change. 0 and 100% are the game's own.");
				}));
		// Shulker boxes exist from 1.11.
		if (ArcticClient.platform() != null && ArcticClient.platform().canPeekShulkers()) {
			out.add(new Mod("shulkerpeek", "Shulker Peek", "Hover a shulker box or chest to see what is inside it", "shulkerpeek",
					Category.GAMEPLAY, "shulker box peek preview contents chest barrel inventory tooltip")
					.toggle(() -> c.shulkerPeek, on -> c.shulkerPeek = on)
					.settings((host, f) -> {
						f.section("Preview");
						f.choice("Show", new String[] {"Always", "While holding a key"},
								() -> c.shulkerPeekMode, i -> c.shulkerPeekMode = i);
						f.section("Keys");
						f.key("Preview key", "Hold it over a container for what is inside, packed",
								Form.key(() -> c.shulkerPeekKey, k -> c.shulkerPeekKey = k));
						f.key("Every slot", "Hold it as well to see every slot where it lies",
								Form.key(() -> c.shulkerPeekFullKey, k -> c.shulkerPeekFullKey = k));
						f.note("Works for shulker boxes (they colour the window), chests, barrels, hoppers and other containers that carry their items. A line next to the cursor tells you the keys.");
					}));
		}
		out.add(new Mod("controller", "Controller", "Play with a gamepad: Xbox, PlayStation and others", "controller",
				Category.GAMEPLAY, "gamepad joystick xbox playstation console")
				.toggle(() -> c.controller.enabled, on -> c.controller.enabled = on)
				.settings((host, f) -> controllerSettings(f, c)));
		out.add(new Mod("tiers", "Tier Tags", "Players' PvP tiers by their names: HT3 Sword | Steve", "tiers", Category.SOCIAL,
				"tier list mctiers pvptiers subtiers rank test")
				.toggle(() -> c.tierTags, on -> c.tierTags = on)
				.settings((host, f) -> tierSettings(f, c)));
		out.add(new Mod("leave", "Confirm Leaving", "Click Disconnect or Save and Quit twice to leave", "leave", Category.GAMEPLAY,
				"quit disconnect")
				.toggle(() -> c.confirmLeave, on -> c.confirmLeave = on));
		out.add(new Mod("reconnect", "Auto Reconnect", "Lost connection? Join the server again by itself", "reconnect", Category.GAMEPLAY,
				"rejoin disconnect kicked timeout connection lost")
				.toggle(() -> c.autoReconnect, on -> c.autoReconnect = on)
				.settings((host, f) -> {
					f.section("Reconnect");
					String[] delays = new String[com.arcticlauncher.client.servers.DisconnectedPage.DELAYS.length];
					for (int i = 0; i < delays.length; i++) {
						delays[i] = com.arcticlauncher.client.servers.DisconnectedPage.duration(com.arcticlauncher.client.servers.DisconnectedPage.DELAYS[i]);
					}
					f.choice("Wait before rejoining", delays,
							() -> Math.max(0, indexOf(com.arcticlauncher.client.servers.DisconnectedPage.DELAYS, c.reconnectDelayMillis())),
							i -> c.reconnectMillis = com.arcticlauncher.client.servers.DisconnectedPage.DELAYS[i]);
					int[] tryChoices = com.arcticlauncher.client.servers.DisconnectedPage.TRIES;
					String[] tries = new String[tryChoices.length];
					for (int i = 0; i < tries.length; i++) {
						tries[i] = tryChoices[i] + (tryChoices[i] == 1 ? " try" : " tries");
					}
					f.choice("Give up after", tries,
							() -> Math.max(0, indexOf(tryChoices, c.reconnectTries)),
							i -> c.reconnectTries = tryChoices[i]);
					f.note("The Reconnect button is there either way. Playing for a minute resets the count.");
				}));
	}

	private static void visuals(List<Mod> out, final ClientConfig c, Platform p) {
		out.add(new Mod("fullbright", "Fullbright", "See in the dark", "fullbright", Category.VISUAL, "gamma brightness night vision")
				.toggle(() -> c.fullbright, on -> c.fullbright = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Fullbright key", "Switches it on and off", Form.key(() -> c.fullbrightKey, k -> c.fullbrightKey = k));
					f.section("Speed");
					f.toggle("Skip light calculations", "Faster: the game stops working out light. Rejoin the world after switching it off",
							() -> c.fullbrightSkipLight, on -> c.fullbrightSkipLight = on);
				}));
		out.add(new Mod("crosshair", "Crosshair", "Your own crosshair: shape, size, color", "crosshair", Category.VISUAL, "aim")
				.toggle(() -> c.crosshair.enabled, on -> c.crosshair.enabled = on)
				.page(CrosshairTab::new));
		out.add(new Mod("time", "Time Changer", "Pick the time of day on your screen", "time", Category.VISUAL, "day night sky")
				.toggle(() -> c.timeLock >= 0, on -> {
					if (on) {
						c.timeLock = c.timeLockOff >= 0 ? c.timeLockOff : TIME_TICKS[1];
					} else {
						c.timeLockOff = c.timeLock;
						c.timeLock = -1;
					}
				})
				.settings((host, f) -> {
					f.section("Time");
					f.choice("Time of day", TIMES, () -> indexOf(TIME_TICKS, c.timeLock), i -> c.timeLock = TIME_TICKS[i]);
					f.note("Only on your screen; the server's time doesn't change.");
				}));
		out.add(new Mod("weather", "Clear Weather", "Hide rain and thunder on your screen", "weather", Category.VISUAL, "rain snow")
				.toggle(() -> c.clearWeather, on -> c.clearWeather = on));
		out.add(new Mod("fire", "Low Fire", "A shorter fire overlay when you're burning", "fire", Category.VISUAL, "burning overlay")
				.toggle(() -> c.lowFire, on -> c.lowFire = on));
		if (p.oldAnimationsWork()) {
			out.add(new Mod("oldanimations", "1.7 Animations", "The old PvP animations", "fire", Category.VISUAL, "blockhit block sword swing 1.7 old")
					.toggle(() -> c.oldBlockHit || c.oldUseSwing || c.oldBow || c.oldSneak || c.oldItemPosition || c.oldThirdPerson, on -> {
						c.oldBlockHit = on;
						c.oldUseSwing = on;
						c.oldBow = on;
						c.oldSneak = on;
						c.oldItemPosition = on;
						c.oldThirdPerson = on;
					})
					.settings((host, f) -> {
						f.section("Hand");
						f.toggle("Swing while blocking", "The sword swings when you hit while holding right-click", () -> c.oldBlockHit,
								on -> c.oldBlockHit = on);
						f.toggle("Swing while eating and drinking", "The hand swings while you use food, potions and a bow",
								() -> c.oldUseSwing, on -> c.oldUseSwing = on);
						f.toggle("Old bow", "The bow stretches along its own length as you draw it", () -> c.oldBow, on -> c.oldBow = on);
						f.toggle("Item position", "Tools, swords and food sit in your hand the 1.7 way", () -> c.oldItemPosition,
								on -> c.oldItemPosition = on);
						f.toggle("Third person", "Players hold their items the 1.7 way, a blocking sword across the body",
								() -> c.oldThirdPerson, on -> c.oldThirdPerson = on);
						f.section("Camera");
						f.toggle("Smooth sneaking", "The view sinks and rises when you sneak, not at once", () -> c.oldSneak,
								on -> c.oldSneak = on);
						f.note("Only what you see: blocking itself works as the server decides.");
					}));
		}
		if (p.motionBlurWorks()) {
			out.add(new Mod("motionblur", "Motion Blur", "A soft blur as you move and turn", "motionblur", Category.VISUAL, "smooth trail")
					.toggle(() -> c.motionBlur, on -> c.motionBlur = on)
					.settings((host, f) -> {
						f.section("Blur");
						f.choice("Mode", BLUR_MODES, () -> Math.max(0, Math.min(BLUR_MODES.length - 1, c.motionBlurMode)),
								i -> c.motionBlurMode = i);
						f.slider("Strength", () -> c.motionBlurAmount, v -> c.motionBlurAmount = (int) v, 0, 100, 1, "%.0f%%");
						f.note("The HUD and menus stay sharp.");
					}));
		}
		if (p.itemPhysicsWorks()) {
			out.add(new Mod("itemphysics", "Item Physics", "Dropped items lie flat and tumble as they fall", "itemphysics",
					Category.VISUAL, "drops ground realistic")
					.toggle(() -> c.itemPhysics, on -> c.itemPhysics = on));
		}
		if (p.outlineTweaks()) {
			out.add(new Mod("outline", "Block Outline", "The outline on the block you look at", "outline", Category.VISUAL, "selection")
					.toggle(() -> c.outlineColor != 0, on -> {
						if (on) {
							c.outlineColor = c.outlineColorOff != 0 ? c.outlineColorOff : OUTLINE_COLORS[0];
							c.outlineWidth = c.outlineWidthOff > 0 ? c.outlineWidthOff : WIDTH_VALUES[1];
						} else {
							c.outlineColorOff = c.outlineColor;
							c.outlineWidthOff = c.outlineWidth;
							c.outlineColor = 0;
							c.outlineWidth = 1f;
						}
					})
					.settings((host, f) -> {
						f.section("Outline");
						f.swatches("Color", OUTLINE_COLORS, () -> c.outlineColor, v -> c.outlineColor = v);
						if (p.outlineWidthWorks()) {
							f.choice("Thickness", WIDTHS, () -> indexOf(WIDTH_VALUES, c.outlineWidth), i -> c.outlineWidth = WIDTH_VALUES[i]);
						}
					}));
		}
		if (p.hitColorWorks()) {
			out.add(new Mod("hitcolor", "Hit Color", "Recolor the red flash players and mobs get when they're hit", "hitcolor", Category.VISUAL,
					"damage red tint")
					.toggle(() -> c.hitColor != 0, on -> {
						if (on) {
							c.hitColor = c.hitColorOff != 0 ? c.hitColorOff : HIT_COLORS[1] & 0xFFFFFF;
						} else {
							c.hitColorOff = c.hitColor;
							c.hitColor = 0;
						}
					})
					.settings((host, f) -> {
						f.section("Color");
						f.swatches("Hit color", HIT_COLORS, () -> 0xFF000000 | c.hitColor, v -> c.hitColor = v & 0xFFFFFF);
					}));
		}
		if (p.scoreboardTweaks()) {
			out.add(new Mod("scoreboard", "Scoreboard", "The sidebar: numbers, background, or hide it", "scoreboard", Category.VISUAL,
					"sidebar")
					.toggle(() -> !c.scoreboardHidden, on -> c.scoreboardHidden = !on)
					.settings((host, f) -> {
						f.section("Scoreboard");
						f.toggle("Numbers", "The red score numbers on the right", () -> c.scoreboardNumbers, on -> c.scoreboardNumbers = on);
						f.toggle("Background", "The dark box behind it", () -> c.scoreboardBackground, on -> c.scoreboardBackground = on);
					}));
		}
		out.add(new Mod("packs", "Packs & Shaders", "Resource packs and shaders from Modrinth", "packs", Category.VISUAL,
				"texture resource pack shader iris modrinth").page(PacksTab::new));
	}

	private static void chat(List<Mod> out, final ClientConfig c) {
		out.add(new Mod("chat", "Chat", "Timestamps, stacked lines, mentions, placeholders", "chat", Category.CHAT,
				"timestamp stack mention ping")
				.settings((host, f) -> {
					f.section("Chat");
					f.toggle("Timestamps", "The time in front of every line", () -> c.chatTimestamps, on -> c.chatTimestamps = on);
					f.toggle("Stack repeated lines", "Show (x3) instead of the same line again", () -> c.chatStack, on -> c.chatStack = on);
					f.toggle("Mentions", "Highlight lines that say your name, with a soft ping", () -> c.chatMentions,
							on -> c.chatMentions = on);
					f.toggle("Placeholders", "Type {x} {y} {z} and it sends your position", () -> c.chatPlaceholders,
							on -> c.chatPlaceholders = on);
					f.note("Also: " + QuickMessages.PLACEHOLDERS);
				}));
		out.add(new Mod("chathistory", "Chat History", "Search every line, filters, a second chat and a longer scrollback", "search",
				Category.CHAT, "search find filter log scrollback unlimited keyword second chat")
				.page(ChatTab::new));
		out.add(new Mod("emoji", "Emoji and Symbols", "Hearts, stars and arrows: pick one, or type :heart:", "chat", Category.CHAT,
				"unicode emoji symbols heart star shortcode")
				.toggle(() -> c.chatEmoji, on -> c.chatEmoji = on)
				.page(EmojiTab::new));
		out.add(new Mod("messages", "Quick Messages", "Keys that send a line or a command", "messages", Category.CHAT, "macro bind")
				.settings(ChatForms::quickMessages));
		out.add(new Mod("autogg", "Auto GG", "Say gg when a minigame ends", "autogg", Category.CHAT, "good game hypixel")
				.toggle(() -> c.autoGg, on -> c.autoGg = on)
				.settings(ChatForms::autoGg));
		out.add(new Mod("streamer", "Streamer Mode", "Hide your name, skin and the server on screen", "streamer", Category.CHAT,
				"privacy snipe hide name")
				.toggle(() -> c.streamerMode, on -> c.streamerMode = on)
				.settings(ChatForms::streamer));
	}

	private static void social(List<Mod> out, ClientConfig c) {
		out.add(new Mod("friends", "Friends", "Who's online, chat, screenshots, voice", "friends", Category.SOCIAL, "voice duel invite")
				.page(FriendsPage::new));
		out.add(new Mod("account", "Account", "Switch accounts, or use a proxy", "account", Category.CLIENT, "proxy login")
				.page(AccountPage::new));
		out.add(new Mod("lookscheck", "Cosmetics Check", "Why can't others see my look? What this game found", "looks", Category.CLIENT,
				"cosmetics skin cape check-in debug missing invisible")
				.page(LooksCheckTab::new));
		out.add(new Mod("style", "Menu Style", "How the menus look; smooth font", "style", Category.CLIENT, "theme font fancy")
				.page(StyleTab::new));
	}

	/** One look for every HUD widget, with the FPS widget as the preview. */
	private static final int[] SPEEDS = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};

	private static void controllerSettings(Form f, final ClientConfig c) {
		f.section("Controller");
		// Checked every frame: a rescan (or a newly plugged-in pad) shows up without reopening the tab.
		f.put(new com.arcticlauncher.client.ui.Label("", com.arcticlauncher.client.ui.Label.Kind.MUTED) {
			@Override
			protected void draw(com.arcticlauncher.client.gfx.Gfx g, com.arcticlauncher.client.style.Style st, int mx, int my, float dt) {
				text(ArcticClient.controller().connected() ? "A controller is connected."
						: "No controller found. Plug one in; on 1.8.9-1.12.2 then press Rescan.");
				super.draw(g, st, mx, my, dt);
			}
		}, f.x + 4, f.row(11), f.w - 4, 11);
		f.action("Controllers", "Rescan", () -> ArcticClient.platform().rescanGamepads());
		f.cycle("Look speed", labels(SPEEDS, ""), () -> c.controller.lookSpeed - 1, i -> c.controller.lookSpeed = i + 1);
		f.cycle("Pointer speed", labels(SPEEDS, ""), () -> c.controller.cursorSpeed - 1, i -> c.controller.cursorSpeed = i + 1);
		f.toggle("Invert looking", "Push up to look down", () -> c.controller.invertY, on -> c.controller.invertY = on);
		f.section("Buttons");
		f.note("Left stick: walk. Right stick: look. A: jump. B: sneak. X: drop. Y: inventory.");
		f.note("RT: attack. LT: use. LB and RB: hotbar. Press the left stick: sprint; the right stick: change view.");
		f.note("View: players. Menu: pause. D-pad up: swap hands; down: chat.");
		f.note("On screens the left stick moves the pointer: A clicks, X right-clicks, B goes back, the right stick scrolls.");
	}

	private static void tierSettings(Form f, final ClientConfig c) {
		f.section("Tier list");
		String[] sources = new String[Tiers.SOURCES.length];
		for (int i = 0; i < sources.length; i++) {
			sources[i] = Tiers.SOURCES[i].name;
		}
		f.choice("From", sources, () -> Math.max(0, Math.min(sources.length - 1, c.tierSource)), i -> {
			c.tierSource = i;
			c.tierMode = "";
		});
		final Tiers.Source source = Tiers.SOURCES[Math.max(0, Math.min(sources.length - 1, c.tierSource))];
		String[] modes = new String[source.modes.length + 1];
		modes[0] = "Their best";
		System.arraycopy(source.titles, 0, modes, 1, source.titles.length);
		f.cycle("Game mode", modes, () -> 1 + java.util.Arrays.asList(source.modes).indexOf(c.tierMode),
				i -> c.tierMode = i == 0 ? "" : source.modes[i - 1]);
		f.section("Show them");
		f.toggle("Name tags", "Above players' heads", () -> c.tierNametags, on -> c.tierNametags = on);
		f.toggle("Tab list", "In the player list", () -> c.tierTab, on -> c.tierTab = on);
		f.note("Players are looked up on " + source.name + " by their account; untested players show nothing.");
	}

	private static Mod hudStyle(final ClientConfig c) {
		final String[][] styles = com.arcticlauncher.client.hud.HudStyles.ALL;
		String[] names = new String[styles.length];
		for (int i = 0; i < styles.length; i++) {
			names[i] = styles[i][1];
		}
		Mod m = new Mod("hudstyle", "HUD Style", "How every HUD widget looks, in one click", "hud", Category.HUD, "theme look lunar")
				.settings((host, f) -> {
					f.section("Style");
					f.choice("Style", names, () -> {
						for (int i = 0; i < styles.length; i++) {
							if (styles[i][0].equals(c.hudStyle)) {
								return i;
							}
						}
						return -1;
					}, i -> {
						c.hudStyle = styles[i][0];
						com.arcticlauncher.client.hud.HudStyles.apply(c.hudStyle, ArcticClient.hud(), ArcticClient.style());
					});
					for (String[] st : styles) {
						f.note(st[1] + ": " + st[2]);
					}
					f.note("Change single widgets on their own pages afterwards.");
				});
		for (HudWidget w : ArcticClient.hud().widgets()) {
			if ("fps".equals(w.id)) {
				m.widget = w;
			}
		}
		return m;
	}

	private static String[] labels(int[] values, String unit) {
		String[] out = new String[values.length];
		for (int i = 0; i < values.length; i++) {
			out[i] = values[i] + unit;
		}
		return out;
	}

	/** Other words a player might search a widget by. */
	private static String widgetKeywords(String id) {
		switch (id) {
			case "effects": return " potion potions status buff buffs debuff";
			case "armor": return " durability gear";
			case "keystrokes": return " wasd keys cps";
			case "arrows": return " bow ammo";
			case "blocking": return " shield sword block parry pvp";
			case "falldamage": return " fall damage height safe landing";
			case "pearl": return " ender pearl cooldown throw";
			case "tnt": return " fuse explode explosion countdown timer";
			case "bossbar": return " boss bar dragon wither";
			case "cpu": case "syscpu": return " processor usage load performance resource";
			case "sysram": case "alloc": return " memory ram usage performance resource";
			case "gpu": case "vram": return " graphics card video memory usage performance resource";
			case "frametime": case "lows": return " stutter lag frame fps percentile performance resource";
			default: return "";
		}
	}

	private static int indexOf(int[] values, int v) {
		for (int i = 0; i < values.length; i++) {
			if (values[i] == v) {
				return i;
			}
		}
		return -1;
	}

	private static int indexOf(float[] values, float v) {
		for (int i = 0; i < values.length; i++) {
			if (values[i] == v) {
				return i;
			}
		}
		return -1;
	}
}
