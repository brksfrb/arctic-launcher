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

import com.arcticlauncher.client.Lang;
/** Every card on the Mods screen, for this version of the game. */
final class ModCatalog {
	private static final int[] OUTLINE_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFF000000};
	/** Unset, the pointer takes the menu's accent color. */
	private static final int[] WATERMARK_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFFB923C, 0xFFF87171, 0xFFE879F9, 0xFFC4B5FD};
	private static final int[] POINTER_COLORS = {0xFF7DD3FC, 0xFFFFFFFF, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9};
	private static final int[] DAMAGE_COLORS = {0xFFE11D48, 0xFFF97316, 0xFFFDE047, 0xFF7DD3FC, 0xFFA78BFA, 0xFFFFFFFF};
	private static final int[] HIT_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF97316, 0xFFE879F9};
	private static final String[] TIMES = {"mods.time.day", "mods.time.noon", "mods.time.sunset", "mods.time.night", "mods.time.midnight"};
	private static final int[] TIME_TICKS = {1000, 6000, 12500, 13500, 18000};
	private static final String[] WIDTHS = {"mods.width.thin", "mods.width.normal", "mods.width.thick"};
	private static final float[] WIDTH_VALUES = {1f, 2f, 3f};
	/** Motion blur strengths, 1 to 4. */
	private static final String[] BLUR_MODES = {"mods.blur_mode.turn_move", "mods.blur_mode.turning", "mods.blur_mode.moving"};

	private ModCatalog() {}

	/** The Mods screen's filter chips for HUD widgets (text keys, also the groups' ids). */
	static final String[] HUD_GROUPS = {"mods.hud_group.all", "mods.hud_group.performance", "mods.hud_group.pvp", "mods.hud_group.world", "mods.hud_group.info"};

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
		group(Category.COMBAT, "combatfx", "tnttimer", "hitboxes", "rangeindicator", "durability", "hitcolor");
		group(Category.INVENTORY, "slotlocks", "invsearch", "invsort", "pickupfeed", "shulkerpeek", "hotbarscroll");
		group(Category.MOVEMENT, "zoom", "freelook", "snaplook", "sprint", "sneak", "portalscreens", "controller");
		group(Category.TOOLS, "quickplay", "waypoints", "screenshots", "replays", "death", "leave", "reconnect");
		group(Category.PLAYERS, "pingtags", "badgetags", "ownnametag", "ofcapes", "tabping", "tabglow", "tiers");
		widgets("mods.hud_group.performance", "fps", "ping", "tps", "frametime", "lows", "cpu", "syscpu", "sysram", "alloc", "gpu", "vram", "memory",
				"entities");
		widgets("mods.hud_group.pvp", "cps", "keystrokes", "armor", "effects", "arrows", "items", "held", "pearl", "blocking", "falldamage", "target");
		widgets("mods.hud_group.world", "coords", "direction", "speed", "biome", "light", "chunk", "day", "worldtime", "clock", "date", "compass",
				"minimap", "death", "stopwatch", "aimed", "horse");
		widgets("mods.hud_group.pvp", "cooldowns");
		for (String id : new String[] {"combatfx", "slotlocks", "invsearch", "invsort", "pickupfeed", "rangeindicator", "shulkerpeek", "particles", "helditem", "palette",
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

	/** The "Light" HUD widget (the old readout), or null. */
	private static HudWidget lightReadout() {
		for (HudWidget w : ArcticClient.hud().widgets()) {
			if ("light".equals(w.id)) {
				return w;
			}
		}
		return null;
	}

	static List<Mod> all() {
		final ClientConfig c = ArcticClient.config();
		Platform p = ArcticClient.platform();
		List<Mod> out = new ArrayList<Mod>();
		boolean game = p.hasFeatures();
		if (game) {
			gameplay(out, c, p);
			visuals(out, c, p);
			chat(out, c);
		}
		social(out, c);
		out.add(hudStyle(c));
		for (final HudWidget w : ArcticClient.hud().widgets()) {
			final HudSlot slot = ArcticClient.hud().slot(w);
			Mod m = new Mod("hud." + w.id, w.name, w.description, w.id, Category.HUD, "hud widget" + widgetKeywords(w.id)) // i18n-ignore
					// Where it was placed is kept while it's off.
					.toggle(() -> slot.enabled, on -> slot.enabled = on)
					.settings((host, f) -> {
						if ("compass".equals(w.id)) {
							f.section(Lang.t("mods.pointer"));
							f.swatches(Lang.t("mods.color"), POINTER_COLORS, () -> c.compassPointer, v -> c.compassPointer = v);
						}
						if ("watermark".equals(w.id)) {
							f.section(Lang.t("mods.watermark"));
							f.choice(Lang.t("mods.corner"), Lang.all(com.arcticlauncher.client.hud.Watermark.CORNERS), () -> c.watermarkCorner, i -> c.watermarkCorner = i);
							f.choice(Lang.t("mods.size"), new String[] {"12", "16", "20", "24", "32"}, () -> c.watermarkSize, i -> c.watermarkSize = i);
							f.choice(Lang.t("mods.opacity"), Lang.all(com.arcticlauncher.client.hud.Watermark.OPACITIES), () -> c.watermarkOpacity, i -> c.watermarkOpacity = i);
							f.toggle(Lang.t("mods.name"), Lang.t("mods.arctic_beside_the_snowflake"), () -> c.watermarkName, on -> c.watermarkName = on);
							f.toggle(Lang.t("mods.version"), Lang.t("mods.the_launchers_version_after_the_snowflake"), () -> c.watermarkVersion, on -> c.watermarkVersion = on);
							f.swatches(Lang.t("mods.text_color"), WATERMARK_COLORS, () -> c.watermarkColor, v -> c.watermarkColor = v);
						}
						if ("minimap".equals(w.id)) {
							f.section(Lang.t("mods.map"));
							f.choice(Lang.t("mods.shape"), new String[] {Lang.t("mods.square"), Lang.t("mods.round")}, () -> c.minimapRound ? 1 : 0, i -> c.minimapRound = i == 1);
							f.toggle(Lang.t("mods.turn_with_me"), Lang.t("mods.the_way_you_face_stays_up_otherwise_north_is"), () -> c.minimapRotate,
									on -> c.minimapRotate = on);
							f.choice(Lang.t("mods.size"), labels(ClientConfig.MINIMAP_SIZES, ""),
									() -> Math.max(0, indexOf(ClientConfig.MINIMAP_SIZES, c.minimapSize)),
									i -> c.minimapSize = ClientConfig.MINIMAP_SIZES[i]);
							f.choice(Lang.t("mods.shows"), labels(ClientConfig.MINIMAP_ZOOMS, " blocks"),
									() -> Math.max(0, indexOf(ClientConfig.MINIMAP_ZOOMS, c.minimapZoom)),
									i -> c.minimapZoom = ClientConfig.MINIMAP_ZOOMS[i]);
							f.toggle(Lang.t("mods.other_players"), Lang.t("mods.arrows_for_the_players_near_you"), () -> c.minimapPlayers, on -> c.minimapPlayers = on);
							f.toggle(Lang.t("mods.coordinates"), Lang.t("mods.your_x_and_z_under_the_map"), () -> c.minimapCoords, on -> c.minimapCoords = on);
						}
						if ("coords".equals(w.id)) {
							f.section(Lang.t("mods.numbers"));
							f.stepper(Lang.t("mods.decimals"), () -> c.coordDecimals, v -> c.coordDecimals = (int) v, 0, 2, 1, "%.0f");
						}
						if ("items".equals(w.id)) {
							f.section(Lang.t("mods.counted_items"));
							f.note(Lang.t("mods.item_ids_with_an_optional_label_separated_by"));
							int top = f.row(Form.ROW);
							final com.arcticlauncher.client.ui.TextField list = new com.arcticlauncher.client.ui.TextField(
									"ender_pearl:Pearls, arrow", 160, com.arcticlauncher.client.ui.TextField.ANY).text(c.countedItems);
							list.onChange(() -> c.countedItems = list.text());
							f.put(list, f.x, top, f.w, Form.ROW - 4);
						}
						if ("stopwatch".equals(w.id)) {
							f.section(Lang.t("mods.key"));
							f.key(Lang.t("mods.stopwatch_key"), Lang.t("mods.starts_stops_and_clears_it"), Form.key(() -> c.stopwatchKey, k -> c.stopwatchKey = k));
						}
						// The watermark has its own text color and no box, shadow or chroma: those would do nothing.
						if (!"watermark".equals(w.id)) {
							HudLookForm.build(host, f, w);
						}
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
				m.group = group == null ? "mods.hud_group.info" : group;
			}
		}
		if (!p.hasScreenHooks()) {
			// This version has no hooks for them: better not offered than switched on and silent.
			List<String> missing = java.util.Arrays.asList("slotlocks", "invsearch", "invsort", "particles", "helditem");
			for (int i = out.size() - 1; i >= 0; i--) {
				if (missing.contains(out.get(i).id)) {
					out.remove(i);
				}
			}
		}
		return out;
	}

	private static void gameplay(List<Mod> out, final ClientConfig c, Platform p) {
		out.add(new Mod("zoom", Lang.t("mods.zoom"), Lang.t("mods.hold_a_key_to_zoom_in_scroll_to_zoom_further"), "zoom", Category.GAMEPLAY, "optifine spyglass") // i18n-ignore
				.toggle(() -> c.zoomEnabled, on -> c.zoomEnabled = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key(Lang.t("mods.zoom_key"), Lang.t("mods.hold_it_to_zoom_scroll_while_zooming_to_go"), Form.key(() -> c.zoomKey, k -> c.zoomKey = k));
					f.section(Lang.t("mods.animation"));
					f.choice(Lang.t("mods.curve"), Lang.all(ClientConfig.ZOOM_CURVES), () -> c.zoomAnimation, i -> c.zoomAnimation = i);
					f.stepper(Lang.t("mods.duration"), () -> c.zoomDurationMs, v -> c.zoomDurationMs = (int) v, 0, 1000, 50, "%.0f ms");
					f.toggle(Lang.t("mods.smooth_scroll_zoom"), Lang.t("mods.the_wheel_eases_to_the_new_zoom_level"),
							() -> c.zoomSmoothScroll, on -> c.zoomSmoothScroll = on);
					f.section(Lang.t("mods.feel"));
					f.toggle(Lang.t("mods.smooth_camera_while_zoomed"), Lang.t("mods.like_optifines_zoom_the_view_glides_instead"),
							() -> c.zoomSmoothCamera, on -> c.zoomSmoothCamera = on);
					f.stepper(Lang.t("mods.zoom_sensitivity"), () -> c.zoomSensitivity, v -> c.zoomSensitivity = (int) v, 10, 200, 5, "%.0f%%");
				}));
		out.add(new Mod("emotes", Lang.t("mods.emotes"), Lang.t("mods.hold_a_key_point_at_an_emote_let_go_others"), "emotes", Category.SOCIAL,
				"emote wheel dance wave key rebind")
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key(Lang.t("mods.emote_wheel_key"), Lang.t("mods.hold_it_move_the_mouse_to_an_emote_let_go"),
							Form.key(() -> c.emoteKey, k -> c.emoteKey = k));
					f.note(Lang.t("mods.pick_your_look_and_emotes_in_arctic_launcher"));
				}));
		if (p.freelookWorks()) {
			out.add(new Mod("freelook", Lang.t("mods.freelook"), Lang.t("mods.hold_a_key_to_look_around_without_turning"), "freelook", Category.GAMEPLAY, "perspective 360")
					.toggle(() -> c.freelookEnabled, on -> c.freelookEnabled = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.key"));
						f.key(Lang.t("mods.freelook_key"), Lang.t("mods.hold_it_to_look_around"), Form.key(() -> c.freelookKey, k -> c.freelookKey = k));
						f.note(Lang.t("mods.some_servers_like_hypixel_dont_allow_it_it"));
					}));
			out.add(new Mod("snaplook", Lang.t("mods.snaplook"), Lang.t("mods.hold_a_key_to_see_yourself_from_the_front"), "snaplook", Category.GAMEPLAY,
					"front view third person behind look back mirror")
					.toggle(() -> c.snaplookEnabled, on -> c.snaplookEnabled = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.key"));
						f.key(Lang.t("mods.snaplook_key"), Lang.t("mods.hold_it_to_see_yourself_from_the_front"), Form.key(() -> c.snaplookKey, k -> c.snaplookKey = k));
					}));
		}
		out.add(new Mod("portalscreens", Lang.t("mods.portal_screens"), Lang.t("mods.keep_the_inventory_chat_and_menus_open_while"), "portalscreens",
				Category.GAMEPLAY, "nether portal gui inventory chat menu close screen")
				.toggle(() -> c.portalScreens, on -> c.portalScreens = on));
		out.add(new Mod("tnttimer", Lang.t("mods.tnt_timer"), Lang.t("mods.the_seconds_left_on_a_lit_tnt_floating_above"), "tnt", Category.GAMEPLAY,
				"tnt explosion fuse countdown timer bomb")
				.toggle(() -> c.tntTimer, on -> c.tntTimer = on));
		out.add(new Mod("hotbarscroll", Lang.t("mods.hotbar_scroll_stop"), Lang.t("mods.scrolling_stops_at_slot_1_and_9_instead_of"), "items", Category.GAMEPLAY,
				"hotbar scroll wheel slot wrap around inventory")
				.toggle(() -> c.hotbarScrollStop, on -> c.hotbarScrollStop = on));
		out.add(new Mod("sprint", Lang.t("mods.toggle_sprint"), Lang.t("mods.press_sprint_once_to_keep_sprinting"), "sprint", Category.GAMEPLAY, "run")
				.toggle(() -> c.toggleSprint, on -> c.toggleSprint = on));
		out.add(new Mod("sneak", Lang.t("mods.toggle_sneak"), Lang.t("mods.press_sneak_once_to_keep_sneaking"), "sneak", Category.GAMEPLAY, "crouch shift") // i18n-ignore
				.toggle(() -> c.toggleSneak, on -> c.toggleSneak = on));
		out.add(new Mod("waypoints", Lang.t("mods.waypoints"), Lang.t("mods.mark_places_and_see_them_in_the_world"), "waypoints", Category.GAMEPLAY, "marker home death")
				.toggle(() -> c.waypointsInWorld, on -> c.waypointsInWorld = on)
				.page(WaypointsTab::new));
		out.add(new Mod("screenshots", Lang.t("mods.screenshots"), Lang.t("mods.copy_new_screenshots_to_the_clipboard"), "camera", Category.GAMEPLAY,
				"clipboard copy f2 picture")
				.toggle(() -> c.copyScreenshots, on -> c.copyScreenshots = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.copy"));
					f.toggle(Lang.t("mods.copy_automatically"), Lang.t("mods.every_new_screenshot_goes_onto_the_clipboard"), () -> c.copyScreenshots,
							on -> c.copyScreenshots = on);
					f.note(Lang.t("mods.when_its_off_the_pop_up_has_a_copy_button"));
					f.note(Lang.t("mods.inventory_to_free_the_mouse_then_click_it"));
				}));
		// Only where this version can record and play replays.
		if (com.arcticlauncher.client.replay.Replays.canWatch()) {
			out.add(new Mod("replays", Lang.t("mods.replays"), Lang.t("mods.instant_clips_start_stop_recording_and"), "replay", Category.GAMEPLAY,
					"replay record clip video camera recording")
					.page(ReplaysTab::new));
		}
		out.add(new Mod("death", Lang.t("mods.death_position"), Lang.t("mods.a_pop_up_with_where_you_died_and_a_death"), "death", Category.GAMEPLAY, "coords")
				.toggle(() -> c.deathNotice, on -> c.deathNotice = on));
		out.add(new Mod("pingtags", Lang.t("mods.ping_tags"), Lang.t("mods.other_players_ping_in_front_of_their_name"), "pingtags", Category.SOCIAL,
				"ping latency nametag lag ms")
				.toggle(() -> c.pingNametags, on -> c.pingNametags = on));
		out.add(new Mod("badgetags", Lang.t("mods.arctic_badge"), Lang.t("mods.your_badge_shared_with_every_arctic_player"), "badgetags", Category.SOCIAL,
				"badge snowflake arctic nametag tag above head")
				.toggle(() -> c.badgeNametags, on -> c.badgeNametags = on)
				.settings((host, f) -> {
					f.badgeStyles();
					f.choice(Lang.t("mods.show_in"), Lang.all(com.arcticlauncher.client.feature.Badge.WHERE), () -> c.badgeWhere, i -> c.badgeWhere = i);
					f.toggle(Lang.t("mods.only_on_others"), Lang.t("mods.no_badge_on_your_own_name"), () -> c.badgeOthersOnly, on -> c.badgeOthersOnly = on);
				}));
		out.add(new Mod("ownnametag", Lang.t("mods.own_name_tag"), Lang.t("mods.see_your_own_name_tag_above_your_head_in"), "ownnametag", Category.SOCIAL,
				"nametag tag above head self own name third person")
				.toggle(() -> c.ownNametag, on -> c.ownNametag = on));
		out.add(new Mod("ofcapes", Lang.t("mods.optifine_capes"), Lang.t("mods.show_other_players_optifine_capes_without"), "capes", Category.SOCIAL,
				"optifine cape capes cloak")
				.toggle(() -> c.optifineCapes, on -> c.optifineCapes = on));
		out.add(new Mod("dynamicfps", Lang.t("mods.dynamic_fps"), Lang.t("mods.a_lower_frame_rate_limit_while_the_game_is"), "dynamicfps", Category.VISUAL,
				"fps limit background unfocused idle afk battery save power performance")
				.toggle(() -> c.dynamicFps, on -> c.dynamicFps = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.limits_0_no_limit_of_that_kind"));
					f.stepper(Lang.t("mods.in_another_window"), () -> c.dynFpsUnfocused, v -> c.dynFpsUnfocused = (int) v, 0, 60, 5, "%.0f FPS");
					f.stepper(Lang.t("mods.idle"), () -> c.dynFpsIdle, v -> c.dynFpsIdle = (int) v, 0, 120, 5, "%.0f FPS");
					f.stepper(Lang.t("mods.idle_after"), () -> c.dynFpsIdleSeconds, v -> c.dynFpsIdleSeconds = (int) v, 10, 600, 10, "%.0f s");
					f.stepper(Lang.t("mods.in_a_menu"), () -> c.dynFpsMenu, v -> c.dynFpsMenu = (int) v, 0, 240, 10, "%.0f FPS");
					f.note(Lang.t("mods.idle_means_nothing_moved_not_you_not_the"));
				}));
		out.add(new Mod("nohurtcam", Lang.t("mods.no_hurt_shake"), Lang.t("mods.the_view_stops_tilting_and_shaking_when_you"), "nohurt", Category.VISUAL,
				"hurt cam camera shake shaking tilt damage bob")
				.toggle(() -> c.noHurtCam, on -> c.noHurtCam = on));
		out.add(new Mod("tabping", Lang.t("mods.tab_ping"), Lang.t("mods.the_ping_in_milliseconds_in_the_player_list"), "tabping", Category.SOCIAL,
				"tab list ping latency ms players")
				.toggle(() -> c.tabPing, on -> c.tabPing = on));
		out.add(new Mod("tabglow", Lang.t("mods.find_me_in_tab"), Lang.t("mods.your_own_name_in_the_player_list_pulses"), "findme", Category.SOCIAL,
				"tab list find myself name highlight glow")
				.toggle(() -> c.tabGlowSelf, on -> c.tabGlowSelf = on));
		if (p.canDuel()) {
			out.add(new Mod("lanauto", Lang.t("mods.auto_lan"), Lang.t("mods.while_you_host_in_arctic_launchers_play"), "friends", Category.SOCIAL,
					"lan open world friends together host share p2p")
					.toggle(() -> c.lanAuto, on -> c.lanAuto = on));
		}
		out.add(new Mod("rangeindicator", Lang.t("mods.attack_range_indicator"), Lang.t("mods.brackets_around_the_crosshair_while_a_player"), "reach",
				Category.GAMEPLAY, "reach range crosshair hit attack distance pvp")
				.toggle(() -> c.rangeIndicator, on -> c.rangeIndicator = on));
		out.add(new Mod("pickupfeed", Lang.t("mods.pickup_display"), Lang.t("mods.items_you_pick_up_are_listed_at_the_right_of"), "items",
				Category.GAMEPLAY, "pickup picked up item loot feed hud collect")
				.toggle(() -> c.pickupFeed, on -> c.pickupFeed = on));
		out.add(new Mod("hitboxes", Lang.t("mods.hitboxes"), Lang.t("mods.every_players_and_mobs_hitbox_as_a_box_like"), "hitboxes",
				Category.GAMEPLAY, "hitbox combat box f3 reach")
				.toggle(() -> c.hitboxes, on -> c.hitboxes = on));
		out.add(new Mod("debugscreen", Lang.t("mods.debug_screen"), Lang.t("mods.a_colorful_f3_screen_labels_and_values_in"), "debugscreen",
				Category.VISUAL, "f3 debug betterf3 coordinates info")
				.toggle(() -> c.debugStyle, on -> c.debugStyle = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.colors"));
					f.choice(Lang.t("mods.labels"), Lang.all(DebugStyle.COLORS), () -> c.debugLabelColor, i -> c.debugLabelColor = i);
					f.choice(Lang.t("mods.values"), Lang.all(DebugStyle.COLORS), () -> c.debugValueColor, i -> c.debugValueColor = i);
					f.section(Lang.t("mods.size"));
					f.stepper(Lang.t("mods.text_size"), () -> c.debugScale, v -> c.debugScale = (int) v, 50, 300, 10, "%.0f%%");
					f.section(Lang.t("mods.lines"));
					for (int g = 0; g < DebugStyle.GROUPS.length; g++) {
						final int group = g;
						f.toggle(Lang.t(DebugStyle.GROUPS[g].label), Lang.t("mods.show_these_lines_on_the_f3_screen"), () -> !DebugStyle.hidden(c, group),
								on -> DebugStyle.setHidden(c, group, !on));
					}
					f.note(Lang.t("mods.press_f3_to_see_it_a_line_this_does_not_know"));
				}));
		out.add(new Mod("saturation", Lang.t("mods.saturation_bar"), Lang.t("mods.your_hidden_saturation_as_a_golden_glow_on"), "food",
				Category.HUD, "hunger food appleskin saturation")
				.toggle(() -> c.saturationBar, on -> c.saturationBar = on));
		out.add(new Mod("durability", Lang.t("mods.low_durability"), Lang.t("mods.a_pop_up_when_armor_or_your_tool_is_about_to"), "durability",
				Category.GAMEPLAY, "armor tool break warning")
				.toggle(() -> c.durabilityWarning, on -> c.durabilityWarning = on));
		out.add(new Mod("combatfx", Lang.t("mods.combat_feedback"), Lang.t("mods.a_hit_marker_hit_and_kill_sounds_and_a_low"), "combatfx",
				Category.GAMEPLAY, "hitmarker kill sound ding low health warning pvp")
				.toggle(() -> Boolean.TRUE.equals(c.combatFeedback), on -> c.combatFeedback = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.hit_marker"));
					f.toggle(Lang.t("mods.hit_marker"), Lang.t("mods.a_small_x_at_your_crosshair_when_you_hit_red"), () -> c.hitMarker,
							on -> c.hitMarker = on);
					f.swatches(Lang.t("mods.color"), CombatFx.MARKER_COLORS, () -> c.hitMarkerColor, i -> c.hitMarkerColor = i);
					f.section(Lang.t("mods.sounds"));
					f.choice(Lang.t("mods.hit_sound"), Lang.all(CombatFx.SOUNDS), () -> c.hitSound, i -> c.hitSound = i);
					f.choice(Lang.t("mods.kill_sound"), Lang.all(CombatFx.SOUNDS), () -> c.killSound, i -> c.killSound = i);
					f.section(Lang.t("mods.low_health"));
					f.toggle(Lang.t("mods.low_health_warning"), Lang.t("mods.a_red_pulse_at_the_screen_edges"), () -> c.lowHealthWarning,
							on -> c.lowHealthWarning = on);
					f.stepper(Lang.t("mods.hearts"), () -> c.lowHealthHearts, v -> c.lowHealthHearts = (int) v, 1, 9, 1, "%.0f");
					f.toggle(Lang.t("mods.warning_beep"), Lang.t("mods.a_soft_beep_while_it_pulses"), () -> c.lowHealthBeep, on -> c.lowHealthBeep = on);
					f.note(Lang.t("mods.a_hit_counts_when_you_attack_the_server_may"));
				}));
		out.add(new Mod("slotlocks", Lang.t("mods.lock_slots"), Lang.t("mods.lock_inventory_slots_so_their_items_cant_be"), "slotlocks",
				Category.GAMEPLAY, "inventory lock protect slot drop hotbar")
				.toggle(() -> c.slotLocks, on -> c.slotLocks = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key(Lang.t("mods.lock_key"), Lang.t("mods.hold_it_and_click_a_slot_to_lock_or_unlock"),
							Form.key(() -> c.slotLockKey, k -> c.slotLockKey = k));
					f.note(Lang.t("mods.a_locked_slot_shows_a_padlock_its_item_cant"));
					f.note(Lang.t("mods.swapped_with_a_number_key_or_thrown_out_with"));
				}));
		out.add(new Mod("invsearch", Lang.t("mods.inventory_search"), Lang.t("mods.ctrl_f_in_a_chest_or_your_inventory_find"), "invsearch",
				Category.GAMEPLAY, "chest search find item inventory shulker")
				.toggle(() -> c.containerSearch, on -> c.containerSearch = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.how_it_works"));
					f.note(Lang.t("mods.press_ctrl_f_in_any_chest_barrel_or_your"));
					f.note(Lang.t("mods.matching_items_get_a_frame_the_others_are"));
					f.note(Lang.t("mods.enter_keeps_the_search_escape_clears_it"));
				}));
		out.add(new Mod("invsort", Lang.t("mods.client_sort"), Lang.t("mods.a_key_in_a_chest_or_your_inventory_sorts"), "invsearch",
				Category.GAMEPLAY, "sort chest inventory organize stack merge shulker")
				.toggle(() -> c.inventorySort, on -> c.inventorySort = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key(Lang.t("mods.sort_key"), Lang.t("mods.hover_the_chest_or_your_inventory_and_press"),
							Form.key(() -> c.inventorySortKey, k -> c.inventorySortKey = k));
					f.note(Lang.t("mods.like_items_are_merged_then_ordered_by_name"));
					f.note(Lang.t("mods.in_your_inventory_only_the_27_slots_above"));
					f.note(Lang.t("mods.it_clicks_one_slot_per_tick_like_you_would"));
				}));
		out.add(new Mod("palette", Lang.t("mods.command_palette"), Lang.t("mods.ctrl_k_search_mods_servers_and_friends_and"), "palette",
				Category.CLIENT, "search command ctrl k quick find launcher")
				.toggle(() -> c.commandPalette, on -> c.commandPalette = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.how_it_works"));
					f.note(Lang.t("mods.press_ctrl_k_anywhere_in_the_game_type_and"));
					f.note(Lang.t("mods.it_finds_mods_and_their_settings_saved"));
				}));
		out.add(new Mod("particles", Lang.t("mods.particle_control"), Lang.t("mods.show_only_a_share_of_the_particles_fewer"), "particles",
				Category.VISUAL, "particles fps performance crit smoke explosion")
				.toggle(() -> c.particleControl, on -> c.particleControl = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.amount"));
					f.stepper(Lang.t("mods.show"), () -> c.particleAmount, v -> c.particleAmount = (int) v, 0, 100, 10, "%.0f%%");
					f.note(Lang.t("mods.100_is_the_games_own_amount_0_shows_none"));
					f.section(Lang.t("mods.kinds_off_never_shown"));
					for (int k = 0; k < com.arcticlauncher.client.feature.ParticleKinds.LABELS.length; k++) {
						final int bit = 1 << k;
						f.toggle(Lang.t(com.arcticlauncher.client.feature.ParticleKinds.LABELS[k]), "",
								() -> (c.particleOff & bit) == 0, on -> c.particleOff = on ? c.particleOff & ~bit : c.particleOff | bit);
					}
				}));
		out.add(new Mod("helditem", Lang.t("mods.held_item"), Lang.t("mods.move_and_resize_your_hands_and_the_item_you"), "helditem",
				Category.VISUAL, "hand item position size view model offset")
				.toggle(() -> c.heldItem, on -> c.heldItem = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.position"));
					f.stepper(Lang.t("mods.right_left"), () -> c.heldX, v -> c.heldX = (int) v, -50, 50, 5, "%.0f");
					f.stepper(Lang.t("mods.up_down"), () -> c.heldY, v -> c.heldY = (int) v, -50, 50, 5, "%.0f");
					f.stepper(Lang.t("mods.away_closer"), () -> c.heldZ, v -> c.heldZ = (int) v, -50, 50, 5, "%.0f");
					f.section(Lang.t("mods.size"));
					f.stepper(Lang.t("mods.size"), () -> c.heldScale, v -> c.heldScale = (int) v, 30, 500, 10, "%.0f%%");
					f.note(Lang.t("mods.look_at_your_hand_in_the_game_to_see_each"));
				}));
		// Shulker boxes exist from 1.11.
		if (ArcticClient.platform() != null && ArcticClient.platform().canPeekShulkers()) {
			out.add(new Mod("shulkerpeek", Lang.t("mods.shulker_peek"), Lang.t("mods.hover_a_shulker_box_or_chest_to_see_what_is"), "shulkerpeek",
					Category.GAMEPLAY, "shulker box peek preview contents chest barrel inventory tooltip")
					.toggle(() -> c.shulkerPeek, on -> c.shulkerPeek = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.preview"));
						f.choice(Lang.t("mods.show"), new String[] {Lang.t("mods.always"), Lang.t("mods.while_holding_a_key")},
								() -> c.shulkerPeekMode, i -> c.shulkerPeekMode = i);
						f.section(Lang.t("mods.keys"));
						f.key(Lang.t("mods.preview_key"), Lang.t("mods.hold_it_over_a_container_for_what_is_inside"),
								Form.key(() -> c.shulkerPeekKey, k -> c.shulkerPeekKey = k));
						f.key(Lang.t("mods.every_slot"), Lang.t("mods.hold_it_as_well_to_see_every_slot_where_it"),
								Form.key(() -> c.shulkerPeekFullKey, k -> c.shulkerPeekFullKey = k));
						f.note(Lang.t("mods.works_for_shulker_boxes_they_colour_the"));
					}));
		}
		out.add(new Mod("controller", Lang.t("mods.controller"), Lang.t("mods.play_with_a_gamepad_xbox_playstation_and"), "controller",
				Category.GAMEPLAY, "gamepad joystick xbox playstation console")
				.toggle(() -> c.controller.enabled, on -> c.controller.setEnabled(on))
				.settings((host, f) -> controllerSettings(f, c)));
		out.add(new Mod("tiers", Lang.t("mods.tier_tags"), Lang.t("mods.players_pvp_tiers_by_their_names_ht3_sword"), "tiers", Category.SOCIAL,
				"tier list mctiers pvptiers subtiers rank test")
				.toggle(() -> c.tierTags, on -> c.tierTags = on)
				.settings((host, f) -> tierSettings(f, c)));
		out.add(new Mod("leave", Lang.t("mods.confirm_leaving"), Lang.t("mods.click_disconnect_or_save_and_quit_twice_to"), "leave", Category.GAMEPLAY,
				"quit disconnect") // i18n-ignore
				.toggle(() -> c.confirmLeave, on -> c.confirmLeave = on));
		out.add(new Mod("reconnect", Lang.t("mods.auto_reconnect"), Lang.t("mods.lost_connection_join_the_server_again_by"), "reconnect", Category.GAMEPLAY,
				"rejoin disconnect kicked timeout connection lost")
				.toggle(() -> c.autoReconnect, on -> c.autoReconnect = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.reconnect"));
					String[] delays = new String[com.arcticlauncher.client.servers.DisconnectedPage.DELAYS.length];
					for (int i = 0; i < delays.length; i++) {
						delays[i] = com.arcticlauncher.client.servers.DisconnectedPage.duration(com.arcticlauncher.client.servers.DisconnectedPage.DELAYS[i]);
					}
					f.choice(Lang.t("mods.wait_before_rejoining"), delays,
							() -> Math.max(0, indexOf(com.arcticlauncher.client.servers.DisconnectedPage.DELAYS, c.reconnectDelayMillis())),
							i -> c.reconnectMillis = com.arcticlauncher.client.servers.DisconnectedPage.DELAYS[i]);
					int[] tryChoices = com.arcticlauncher.client.servers.DisconnectedPage.TRIES;
					String[] tries = new String[tryChoices.length];
					for (int i = 0; i < tries.length; i++) {
						tries[i] = tryChoices[i] + (tryChoices[i] == 1 ? " try" : " tries");
					}
					f.choice(Lang.t("mods.give_up_after"), tries,
							() -> Math.max(0, indexOf(tryChoices, c.reconnectTries)),
							i -> c.reconnectTries = tryChoices[i]);
					f.note(Lang.t("mods.the_reconnect_button_is_there_either_way"));
				}));
	}

	private static void visuals(List<Mod> out, final ClientConfig c, Platform p) {
		out.add(new Mod("connected-glass", Lang.t("mods.connected_glass"), Lang.t("mods.join_matching_glass_borders"), "fullbright", Category.VISUAL, "ctm textures panes stained tinted video looks clarity streaks lines clear")
				.toggle(() -> c.connectedGlass, on -> c.connectedGlass = on)
				.settings((host, f) -> {
					f.note(Lang.t("mods.connects_matching_glass_and_panes_applies"));
					f.section(Lang.t("mods.glass"));
					f.slider(Lang.t("mods.lines_in_the_glass"), () -> c.clearGlassLines, v -> c.clearGlassLines = (int) Math.round(v), 0, 100, 5, "%.0f%%");
					f.glassPreview(() -> c.clearGlassLines);
					f.note(Lang.t("mods.fades_the_streaks_in_clear_and_stained_glass"));
					f.note(Lang.t("mods.works_with_connected_glass_on_or_off"));
					f.note(Lang.t("mods.applies_a_moment_after_you_let_go_without_a"));
				}));
		out.add(new Mod("fullbright", Lang.t("mods.fullbright"), Lang.t("mods.see_in_the_dark"), "fullbright", Category.VISUAL, "gamma brightness night vision")
				.toggle(() -> c.fullbright, on -> c.fullbright = on)
				.ownToggleKey()
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key(Lang.t("mods.fullbright_key"), Lang.t("mods.switches_it_on_and_off"), Form.key(() -> c.fullbrightKey, k -> c.fullbrightKey = k));
					f.section(Lang.t("mods.speed"));
					f.toggle(Lang.t("mods.skip_light_calculations"), Lang.t("mods.faster_the_game_stops_working_out_light"),
							() -> c.fullbrightSkipLight, on -> c.fullbrightSkipLight = on);
				}));
		out.add(new Mod("crosshair", Lang.t("mods.crosshair"), Lang.t("mods.your_own_crosshair_shape_size_color_2"), "crosshair", Category.VISUAL, "aim")
				.toggle(() -> c.crosshair.enabled, on -> c.crosshair.enabled = on)
				.page(CrosshairTab::new));
		out.add(new Mod("time", Lang.t("mods.time_changer"), Lang.t("mods.pick_the_time_of_day_on_your_screen"), "time", Category.VISUAL, "day night sky")
				.toggle(() -> c.timeLock >= 0, on -> {
					if (on) {
						c.timeLock = c.timeLockOff >= 0 ? c.timeLockOff : TIME_TICKS[1];
					} else {
						c.timeLockOff = c.timeLock;
						c.timeLock = -1;
					}
				})
				.settings((host, f) -> {
					f.section(Lang.t("mods.time"));
					f.choice(Lang.t("mods.time_of_day"), Lang.all(TIMES), () -> indexOf(TIME_TICKS, c.timeLock), i -> c.timeLock = TIME_TICKS[i]);
					f.note(Lang.t("mods.only_on_your_screen_the_servers_time_doesnt"));
				}));
		out.add(new Mod("weather", Lang.t("mods.clear_weather"), Lang.t("mods.hide_rain_and_thunder_on_your_screen"), "weather", Category.VISUAL, "rain snow") // i18n-ignore
				.toggle(() -> c.clearWeather, on -> c.clearWeather = on));
		out.add(new Mod("fire", Lang.t("mods.low_fire"), Lang.t("mods.a_shorter_fire_overlay_when_youre_burning"), "fire", Category.VISUAL, "burning overlay") // i18n-ignore
				.toggle(() -> c.lowFire, on -> c.lowFire = on));
		if (p.oldAnimationsWork()) {
			out.add(new Mod("oldanimations", "1.7 Animations", Lang.t("mods.the_old_pvp_animations"), "oldanim", Category.VISUAL, "blockhit block sword swing 1.7 old") // i18n-ignore
					.toggle(() -> c.oldBlockHit || c.oldUseSwing || c.oldBow || c.oldSneak || c.oldItemPosition || c.oldThirdPerson, on -> {
						c.oldBlockHit = on;
						c.oldUseSwing = on;
						c.oldBow = on;
						c.oldSneak = on;
						c.oldItemPosition = on;
						c.oldThirdPerson = on;
					})
					.settings((host, f) -> {
						f.section(Lang.t("mods.hand"));
						f.toggle(Lang.t("mods.swing_while_blocking"), Lang.t("mods.the_sword_swings_when_you_hit_while_holding"), () -> c.oldBlockHit,
								on -> c.oldBlockHit = on);
						f.toggle(Lang.t("mods.swing_while_eating_and_drinking"), Lang.t("mods.the_hand_swings_while_you_use_food_potions"),
								() -> c.oldUseSwing, on -> c.oldUseSwing = on);
						f.toggle(Lang.t("mods.old_bow"), Lang.t("mods.the_bow_stretches_along_its_own_length_as"), () -> c.oldBow, on -> c.oldBow = on);
						f.toggle(Lang.t("mods.item_position"), Lang.t("mods.tools_swords_and_food_sit_in_your_hand_the_1"), () -> c.oldItemPosition,
								on -> c.oldItemPosition = on);
						f.toggle(Lang.t("mods.third_person"), Lang.t("mods.players_hold_their_items_the_1_7_way_a"),
								() -> c.oldThirdPerson, on -> c.oldThirdPerson = on);
						f.section(Lang.t("mods.camera"));
						f.toggle(Lang.t("mods.smooth_sneaking"), Lang.t("mods.the_view_sinks_and_rises_when_you_sneak_not"), () -> c.oldSneak,
								on -> c.oldSneak = on);
						f.note(Lang.t("mods.only_what_you_see_blocking_itself_works_as"));
					}));
		}
		if (p.motionBlurWorks()) {
			out.add(new Mod("motionblur", Lang.t("mods.motion_blur"), Lang.t("mods.a_soft_blur_as_you_move_and_turn"), "motionblur", Category.VISUAL, "smooth trail") // i18n-ignore
					.toggle(() -> c.motionBlur, on -> c.motionBlur = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.blur"));
						f.choice(Lang.t("mods.mode"), Lang.all(BLUR_MODES), () -> Math.max(0, Math.min(BLUR_MODES.length - 1, c.motionBlurMode)),
								i -> c.motionBlurMode = i);
						f.slider(Lang.t("mods.strength"), () -> c.motionBlurAmount, v -> c.motionBlurAmount = (int) v, 0, 100, 1, "%.0f%%");
						f.note(Lang.t("mods.the_hud_and_menus_stay_sharp"));
					}));
		}
		if (p.itemPhysicsWorks()) {
			out.add(new Mod("itemphysics", Lang.t("mods.item_physics"), Lang.t("mods.dropped_items_lie_flat_and_tumble_as_they"), "itemphysics",
					Category.VISUAL, "drops ground realistic")
					.toggle(() -> c.itemPhysics, on -> c.itemPhysics = on));
		}
		if (p.outlineTweaks()) {
			out.add(new Mod("outline", Lang.t("mods.block_outline"), Lang.t("mods.the_outline_on_the_block_you_look_at"), "outline", Category.VISUAL, "selection")
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
						f.section(Lang.t("mods.outline"));
						f.swatches(Lang.t("mods.color"), OUTLINE_COLORS, () -> c.outlineColor, v -> c.outlineColor = v);
						if (p.outlineWidthWorks()) {
							f.choice(Lang.t("mods.thickness"), Lang.all(WIDTHS), () -> indexOf(WIDTH_VALUES, c.outlineWidth), i -> c.outlineWidth = WIDTH_VALUES[i]);
						}
					}));
		}
		if (p.hitColorWorks()) {
			out.add(new Mod("hitcolor", Lang.t("mods.hit_color"), Lang.t("mods.recolor_the_red_flash_players_and_mobs_get"), "hitcolor", Category.VISUAL,
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
						f.section(Lang.t("mods.color"));
						f.swatches(Lang.t("mods.hit_color_2"), HIT_COLORS, () -> 0xFF000000 | c.hitColor, v -> c.hitColor = v & 0xFFFFFF);
					}));
		}
		if (p.chunkBordersWorks()) {
			out.add(new Mod("chunkborders", Lang.t("mods.chunk_borders"), Lang.t("mods.the_chunk_edges_drawn_as_lines_like_f3_g_in"), "target", Category.GAMEPLAY,
					"chunk border grid f3 g slime lines")
					.toggle(() -> c.chunkBorders, on -> c.chunkBorders = on));
		}
		if (p.lightOverlayWorks()) {
			out.add(new Mod("lightoverlay", Lang.t("mods.light_overlay"), Lang.t("mods.marks_the_ground_where_hostile_mobs_can"),
					"target", Category.GAMEPLAY, "light level lighty spawn mob spawning dark cave lit")
					.toggle(() -> c.lightOverlay, on -> c.lightOverlay = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.overlay"));
						f.note(Lang.t("mods.red_mobs_spawn_here_now_yellow_only_at_night"));
						f.slider(Lang.t("mods.range"), () -> c.lightRange, v -> c.lightRange = (int) v, 4, 32, 1, "%.0f");
						f.choice(Lang.t("mods.style"), Lang.all(com.arcticlauncher.client.feature.LightOverlay.STYLES), () -> c.lightStyle, i -> c.lightStyle = i);
						f.toggle(Lang.t("mods.show_safe_blocks"), Lang.t("mods.green_markers_where_mobs_cant_spawn"), () -> c.lightShowSafe, on -> c.lightShowSafe = on);
						f.key(Lang.t("mods.toggle_key"), Lang.t("mods.switches_the_overlay_on_and_off"), Form.key(() -> c.lightKey, k -> c.lightKey = k));
						f.section(Lang.t("mods.readout"));
						HudWidget readout = lightReadout();
						if (readout != null) {
							final HudSlot slot = ArcticClient.hud().slot(readout);
							f.toggle(Lang.t("mods.light_readout"), Lang.t("mods.keep_the_light_8_sky_0_line_on_screen"), () -> slot.enabled, on -> slot.enabled = on);
						}
					}));
		}
		if (p.tooltipScrollWorks()) {
			out.add(new Mod("tooltipscroll", Lang.t("mods.scrollable_tooltips"), Lang.t("mods.a_tooltip_too_tall_for_the_screen_long_item"), "tooltips",
					Category.GAMEPLAY, "tooltip lore scroll long item wheel skyblock")
					.toggle(() -> c.tooltipScroll, on -> c.tooltipScroll = on));
		}
		if (p.glintWorks()) {
			out.add(new Mod("glint", Lang.t("mods.glint_colorizer"), Lang.t("mods.the_shimmer_on_enchanted_items_and_armor_in"), "glint",
					Category.VISUAL, "enchant enchantment glint shimmer color purple")
					.toggle(() -> c.glintColor != 0, on -> {
						if (on) {
							c.glintColor = c.glintColorOff != 0 ? c.glintColorOff : DAMAGE_COLORS[3] & 0xFFFFFF;
						} else {
							c.glintColorOff = c.glintColor;
							c.glintColor = 0;
						}
					})
					.settings((host, f) -> {
						f.section(Lang.t("mods.color"));
						f.swatches(Lang.t("mods.glint_color"), DAMAGE_COLORS, () -> 0xFF000000 | c.glintColor, v -> c.glintColor = v & 0xFFFFFF);
					}));
		}
		out.add(new Mod("shinypots", Lang.t("mods.shiny_pots"), Lang.t("mods.every_potion_bottle_water_bottles_and_thrown"), "effects", Category.VISUAL,
				"potion pots shiny glint shimmer enchant bottle")
				.toggle(() -> c.shinyPots, on -> c.shinyPots = on));
		out.add(new Mod("items2d", "2D Items", Lang.t("mods.dropped_items_are_flat_sprites_that_always"), "items", Category.VISUAL,
				"items 2d flat sprite drops dropped")
				.toggle(() -> c.items2d, on -> c.items2d = on));
		out.add(new Mod("teamview", Lang.t("mods.team_view"), Lang.t("mods.players_on_your_scoreboard_team_are_outlined"), "players", Category.SOCIAL,
				"team teammates outline glow esp bedwars through walls")
				.toggle(() -> c.teamView, on -> c.teamView = on));
		if (p.dynamicFovWorks()) {
			out.add(new Mod("dynfov", Lang.t("mods.dynamic_fov"), Lang.t("mods.keep_the_view_from_changing_with_sprinting"), "dynfov", Category.VISUAL,
					"fov dynamic field of view sprint speed potion bow optifine")
					.settings((host, f) -> {
						f.toggle(Lang.t("mods.no_speed_zoom"), Lang.t("mods.sprinting_speed_potions_and_flying_leave_the"), () -> c.fovNoSpeed, on -> c.fovNoSpeed = on);
						f.toggle(Lang.t("mods.no_bow_zoom"), Lang.t("mods.drawing_a_bow_leaves_the_view_alone"), () -> c.fovNoBow, on -> c.fovNoBow = on);
					}));
		}
		if (p.fogWorks()) {
			out.add(new Mod("fog", Lang.t("mods.fog"), Lang.t("mods.push_the_fog_away_or_remove_it"), "fog", Category.VISUAL, "fog mist distance clear view")
					.toggle(() -> c.fog, on -> c.fog = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.fog"));
						f.toggle(Lang.t("mods.no_fog_at_all"), Lang.t("mods.pushes_the_fog_out_of_sight"), () -> c.fogOff, on -> c.fogOff = on);
						f.slider(Lang.t("mods.distance"), () -> c.fogScale, v -> c.fogScale = v, 1.0, 10.0, 0.5, "%.1fx");
					}));
		}
		out.add(new Mod("mobsize", Lang.t("mods.mob_size"), Lang.t("mods.draw_every_creature_but_the_players_bigger"), "mobsize", Category.VISUAL,
				"mob size scale big small tiny giant creatures animals")
				.toggle(() -> c.mobSize, on -> c.mobSize = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.size"));
					f.slider(Lang.t("mods.scale"), () -> c.mobSizeFactor, v -> c.mobSizeFactor = v, 0.25, 3.0, 0.05, "%.2fx");
				}));
		out.add(new Mod("titles", Lang.t("mods.hide_titles"), Lang.t("mods.the_big_title_and_subtitle_in_the_middle_of"), "titles", Category.VISUAL,
				"title subtitle screen text announcement hide")
				.toggle(() -> c.hideTitles, on -> c.hideTitles = on));
		out.add(new Mod("soundchanger", Lang.t("mods.sound_changer"), Lang.t("mods.make_single_sounds_quieter_or_silent"), "sounds", Category.GAMEPLAY,
				"sound volume mute footstep hurt quiet noise audio")
				.toggle(() -> c.soundChanger, on -> c.soundChanger = on)
				.settings(SoundForms::build));
		out.add(new Mod("damagetint", Lang.t("mods.damage_tint"), Lang.t("mods.the_screen_flushes_with_a_color_for_a_moment"), "damagetint", Category.VISUAL,
				"hurt red vignette flash damage screen")
				.toggle(() -> c.damageTint, on -> c.damageTint = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.look"));
					f.swatches(Lang.t("mods.color"), DAMAGE_COLORS, () -> 0xFF000000 | c.damageTintColor, v -> c.damageTintColor = v & 0xFFFFFF);
					f.slider(Lang.t("mods.strength"), () -> c.damageTintStrength, v -> c.damageTintStrength = v, 0.1, 1.0, 0.05, "%.2f");
				}));
		if (p.scoreboardTweaks()) {
			out.add(new Mod("scoreboard", Lang.t("mods.scoreboard"), Lang.t("mods.the_sidebar_numbers_background_or_hide_it"), "scoreboard", Category.VISUAL,
					"sidebar")
					.toggle(() -> !c.scoreboardHidden, on -> c.scoreboardHidden = !on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.scoreboard"));
						f.toggle(Lang.t("mods.numbers"), Lang.t("mods.the_red_score_numbers_on_the_right"), () -> c.scoreboardNumbers, on -> c.scoreboardNumbers = on);
						f.toggle(Lang.t("mods.background"), Lang.t("mods.the_dark_box_behind_it"), () -> c.scoreboardBackground, on -> c.scoreboardBackground = on);
					}));
		}
		out.add(new Mod("packs", Lang.t("mods.packs_shaders"), Lang.t("mods.resource_packs_and_shaders_from_modrinth"), "packs", Category.VISUAL,
				"texture resource pack shader iris modrinth").page(PacksTab::new));
	}

	private static void chatField(Form f, String label, String hint, java.util.function.Supplier<String> get,
			final java.util.function.Consumer<String> set) {
		int top = f.row(Form.ROW);
		f.put(new com.arcticlauncher.client.ui.Label(label, com.arcticlauncher.client.ui.Label.Kind.TEXT), f.x + 4, top, 110, Form.ROW);
		final com.arcticlauncher.client.ui.TextField field = new com.arcticlauncher.client.ui.TextField(hint, 200,
				com.arcticlauncher.client.ui.TextField.ANY).text(get.get());
		field.onChange(() -> set.accept(field.text()));
		f.put(field, f.x + 118, top, f.w - 118, Form.ROW - 4);
	}

	private static void chat(List<Mod> out, final ClientConfig c) {
		out.add(new Mod("chat", Lang.t("mods.chat"), Lang.t("mods.timestamps_stacked_lines_mentions"), "chat", Category.CHAT,
				"timestamp stack mention ping copy clipboard")
				.settings((host, f) -> {
					f.section(Lang.t("mods.chat"));
					f.toggle(Lang.t("mods.copy_chat_messages"), Lang.t("mods.click_the_timestamp_or_copy_shift_right"), () -> c.chatCopy, on -> c.chatCopy = on);
					f.toggle(Lang.t("mods.timestamps"), Lang.t("mods.the_time_in_front_of_every_line"), () -> c.chatTimestamps, on -> c.chatTimestamps = on);
					f.toggle(Lang.t("mods.stack_repeated_lines"), Lang.t("mods.show_x3_instead_of_the_same_line_again"), () -> c.chatStack, on -> c.chatStack = on);
					f.toggle(Lang.t("mods.mentions"), Lang.t("mods.highlight_lines_that_say_your_name_with_a"), () -> c.chatMentions,
							on -> c.chatMentions = on);
					f.toggle(Lang.t("mods.placeholders"), Lang.t("mods.type_and_it_sends_your_position"), () -> c.chatPlaceholders,
							on -> c.chatPlaceholders = on);
					f.note(Lang.t("mods.also") + QuickMessages.PLACEHOLDERS);
					f.section(Lang.t("mods.stacking"));
					f.toggle(Lang.t("mods.only_within_a_time_frame"), Lang.t("mods.same_line_again_after_a_pause_starts_a_new"), () -> c.chatStackTimed,
							on -> c.chatStackTimed = on);
					f.stepper(Lang.t("mods.time_frame_seconds"), () -> c.chatStackSeconds, v -> c.chatStackSeconds = (int) v, 1, 60, 1, "%.0f");
					f.toggle(Lang.t("mods.ignore_blank_lines"), Lang.t("mods.empty_lines_are_never_stacked"), () -> c.chatStackIgnoreBlank,
							on -> c.chatStackIgnoreBlank = on);
					f.toggle(Lang.t("mods.ignore_dividers"), Lang.t("mods.lines_like_are_never_stacked"), () -> c.chatStackIgnoreBreak,
							on -> c.chatStackIgnoreBreak = on);
					f.section(Lang.t("mods.timestamp"));
					f.toggle(Lang.t("mods.12_hour_clock"), Lang.t("mods.3_07_instead_of_15_07"), () -> c.chatStamp12h, on -> c.chatStamp12h = on);
					f.toggle(Lang.t("mods.am_pm"), Lang.t("mods.add_am_or_pm_after_the_time"), () -> c.chatStampAmPm, on -> c.chatStampAmPm = on);
					f.toggle(Lang.t("mods.seconds"), Lang.t("mods.3_07_12_instead_of_3_07"), () -> c.chatStampSeconds, on -> c.chatStampSeconds = on);
					f.toggle(Lang.t("mods.brackets"), Lang.t("mods.3_07_instead_of_3_07"), () -> c.chatStampBrackets, on -> c.chatStampBrackets = on);
					f.toggle(Lang.t("mods.bold"), Lang.t("mods.the_time_in_bold"), () -> c.chatStampBold, on -> c.chatStampBold = on);
					f.toggle(Lang.t("mods.italic"), Lang.t("mods.the_time_in_italics"), () -> c.chatStampItalic, on -> c.chatStampItalic = on);
					f.section(Lang.t("mods.show"));
					f.toggle(Lang.t("mods.hide_the_chat"), Lang.t("mods.no_chat_lines_on_screen_you_can_still_type"), () -> c.chatOff,
							on -> c.chatOff = on);
					f.section(Lang.t("mods.keep"));
					f.toggle(Lang.t("mods.save_chat_across_restarts"), Lang.t("mods.keep_all_messages_and_sent_history_in_this"), () -> c.chatPersistent,
							on -> c.chatPersistent = on);
					f.action(Lang.t("mods.saved_chat"), Lang.t("mods.clear_saved_chat"), Lang.t("mods.sure"), () -> {
						com.arcticlauncher.client.feature.PersistentChat saved = ArcticClient.persistentChat();
						int removed = saved == null ? 0 : saved.clear();
						com.arcticlauncher.client.notice.Notices.post(Lang.t("mods.saved_chat_cleared"), removed == 0 ? Lang.t("mods.nothing_was_saved") : Lang.t("mods.deleted_the_saved_chat_and_sent_history"));
					});
					f.toggle(Lang.t("mods.keep_chat_when_leaving"), Lang.t("mods.the_lines_stay_when_you_disconnect_or_change"), () -> c.chatKeep,
							on -> c.chatKeep = on);
					f.section(Lang.t("mods.mute"));
					f.note(Lang.t("mods.players_whose_lines_never_show_and_words"));
					chatField(f, Lang.t("mods.muted_players"), Lang.t("mods.steve_alex"), () -> c.chatMuted, v -> c.chatMuted = v);
					chatField(f, Lang.t("mods.hidden_words"), "spam, lag", () -> c.chatHiddenWords, v -> c.chatHiddenWords = v);
				}));
		out.add(new Mod("chathistory", Lang.t("mods.chat_history"), Lang.t("mods.search_every_line_filters_a_second_chat_and"), "search",
				Category.CHAT, "search find filter log scrollback unlimited keyword second chat")
				.page(ChatTab::new));
		out.add(new Mod("emoji", Lang.t("mods.emoji_and_symbols_2"), Lang.t("mods.hearts_stars_and_arrows_pick_one_or_type"), "emoji", Category.CHAT,
				"unicode emoji symbols heart star shortcode")
				.toggle(() -> c.chatEmoji, on -> c.chatEmoji = on)
				.page(EmojiTab::new));
		out.add(new Mod("messages", Lang.t("mods.quick_messages"), Lang.t("mods.keys_that_send_a_line_or_a_command"), "messages", Category.CHAT, "macro bind") // i18n-ignore
				.settings(ChatForms::quickMessages));
		out.add(new Mod("quickplay", Lang.t("mods.quickplay"), Lang.t("mods.join_a_minigame_with_a_click_bed_wars"), "quickplay", Category.CHAT,
				"minigame play lobby bedwars skywars duels hypixel queue")
				.settings(QuickplayForm::build));
		out.add(new Mod("autotext", Lang.t("mods.auto_text"), Lang.t("mods.answer_a_chat_line_by_itself_when_it_says_x"), "autotext", Category.CHAT,
				"auto reply answer trigger macro respond")
				.toggle(() -> c.autoReply, on -> c.autoReply = on)
				.settings(ChatForms::autoText));
		out.add(new Mod("autogg", Lang.t("mods.auto_gg"), Lang.t("mods.say_gg_when_a_minigame_ends"), "autogg", Category.CHAT, "good game hypixel")
				.toggle(() -> c.autoGg, on -> c.autoGg = on)
				.settings(ChatForms::autoGg));
		out.add(new Mod("streamer", Lang.t("mods.streamer_mode"), Lang.t("mods.hide_your_name_skin_and_the_server_on_screen"), "streamer", Category.CHAT,
				"privacy snipe hide name")
				.toggle(() -> c.streamerMode, on -> c.streamerMode = on)
				.ownToggleKey()
				.settings(ChatForms::streamer));
	}

	private static void social(List<Mod> out, ClientConfig c) {
		out.add(new Mod("friends", Lang.t("mods.friends"), Lang.t("mods.whos_online_chat_screenshots_voice"), "friends", Category.SOCIAL, "voice duel invite")
				.page(FriendsPage::new));
		out.add(new Mod("account", Lang.t("mods.account"), Lang.t("mods.switch_accounts_or_use_a_proxy"), "account", Category.CLIENT, "proxy login") // i18n-ignore
				.page(AccountPage::new));
		out.add(new Mod("lookscheck", Lang.t("mods.cosmetics_check"), Lang.t("mods.why_cant_others_see_my_look_what_this_game"), "looks", Category.CLIENT,
				"cosmetics skin cape check-in debug missing invisible") // i18n-ignore
				.page(LooksCheckTab::new));
		out.add(new Mod("style", Lang.t("mods.menu_style"), Lang.t("mods.how_the_menus_look_smooth_font"), "style", Category.CLIENT, "theme font fancy")
				.page(StyleTab::new));
	}

	/** One look for every HUD widget, with the FPS widget as the preview. */
	private static final int[] SPEEDS = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};

	private static void controllerSettings(Form f, final ClientConfig c) {
		f.section(Lang.t("mods.controller"));
		// Checked every frame: a rescan (or a newly plugged-in pad) shows up without reopening the tab.
		f.put(new com.arcticlauncher.client.ui.Label("", com.arcticlauncher.client.ui.Label.Kind.MUTED) {
			@Override
			protected void draw(com.arcticlauncher.client.gfx.Gfx g, com.arcticlauncher.client.style.Style st, int mx, int my, float dt) {
				text(ArcticClient.controller().connected() ? Lang.t("mods.a_controller_is_connected")
						: Lang.t("mods.no_controller_found_plug_one_in_on_1_8_9_1_2"));
				super.draw(g, st, mx, my, dt);
			}
		}, f.x + 4, f.row(11), f.w - 4, 11);
		f.action(Lang.t("mods.controllers"), Lang.t("mods.rescan"), () -> ArcticClient.platform().rescanGamepads());
		f.cycle(Lang.t("mods.look_speed"), labels(SPEEDS, ""), () -> c.controller.lookSpeed - 1, i -> c.controller.lookSpeed = i + 1);
		f.cycle(Lang.t("mods.pointer_speed"), labels(SPEEDS, ""), () -> c.controller.cursorSpeed - 1, i -> c.controller.cursorSpeed = i + 1);
		f.toggle(Lang.t("mods.invert_looking"), Lang.t("mods.push_up_to_look_down"), () -> c.controller.invertY, on -> c.controller.invertY = on);
		f.section(Lang.t("mods.buttons"));
		f.note(Lang.t("mods.left_stick_walk_right_stick_look_a_jump_b"));
		f.note(Lang.t("mods.rt_attack_lt_use_lb_and_rb_hotbar_press_the"));
		f.note(Lang.t("mods.view_players_menu_pause_d_pad_up_swap_hands"));
		f.note(Lang.t("mods.on_screens_the_left_stick_moves_the_pointer"));
	}

	private static void tierSettings(Form f, final ClientConfig c) {
		f.section(Lang.t("mods.tier_list"));
		String[] sources = new String[Tiers.SOURCES.length];
		for (int i = 0; i < sources.length; i++) {
			sources[i] = Tiers.SOURCES[i].label();
		}
		f.choice(Lang.t("mods.from"), sources, () -> Math.max(0, Math.min(sources.length - 1, c.tierSource)), i -> {
			c.tierSource = i;
			c.tierMode = "";
		});
		final Tiers.Source source = Tiers.SOURCES[Math.max(0, Math.min(sources.length - 1, c.tierSource))];
		String[] modes = new String[source.modes.length + 1];
		modes[0] = Lang.t("mods.their_best");
		System.arraycopy(source.titles, 0, modes, 1, source.titles.length);
		f.cycle(Lang.t("mods.game_mode"), modes, () -> 1 + java.util.Arrays.asList(source.modes).indexOf(c.tierMode),
				i -> c.tierMode = i == 0 ? "" : source.modes[i - 1]);
		f.section(Lang.t("mods.show_them"));
		f.toggle(Lang.t("mods.name_tags"), Lang.t("mods.above_players_heads"), () -> c.tierNametags, on -> c.tierNametags = on);
		f.toggle(Lang.t("mods.tab_list"), Lang.t("mods.in_the_player_list"), () -> c.tierTab, on -> c.tierTab = on);
		f.note(Lang.t("mods.players_are_looked_up_on") + source.label() + Lang.t("mods.by_their_account_untested_players_show"));
	}

	private static Mod hudStyle(final ClientConfig c) {
		final String[][] styles = com.arcticlauncher.client.hud.HudStyles.ALL;
		String[] names = new String[styles.length];
		for (int i = 0; i < styles.length; i++) {
			names[i] = Lang.t(styles[i][1]);
		}
		Mod m = new Mod("hudstyle", Lang.t("mods.hud_style"), Lang.t("mods.how_every_hud_widget_looks_in_one_click"), "hud", Category.HUD, "theme look lunar")
				.settings((host, f) -> {
					f.section(Lang.t("mods.style"));
					f.choice(Lang.t("mods.style"), names, () -> {
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
						f.note(Lang.t(st[1]) + ": " + Lang.t(st[2]));
					}
					f.note(Lang.t("mods.change_single_widgets_on_their_own_pages"));
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
			case "armor": return " durability gear"; // i18n-ignore
			case "keystrokes": return " wasd keys cps";
			case "arrows": return " bow ammo"; // i18n-ignore
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
