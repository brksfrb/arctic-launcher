package com.arcticlauncher.client.menu;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.feature.QuickMessages;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.menu.Mod.Category;
import com.arcticlauncher.client.tiers.Tiers;

import com.arcticlauncher.client.Lang;
/** Every card on the Mods screen, for this version of the game. */
final class ModCatalog {
	private static final int[] OUTLINE_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFF000000};
	/** Unset, the pointer takes the menu's accent color. */
	private static final int[] POINTER_COLORS = {0xFF7DD3FC, 0xFFFFFFFF, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9};
	private static final int[] HIT_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF97316, 0xFFE879F9};
	private static final String[] TIMES = {"mods.time.day", "mods.time.noon", "mods.time.sunset", "mods.time.night", "mods.time.midnight"};
	private static final int[] TIME_TICKS = {1000, 6000, 12500, 13500, 18000};
	private static final String[] WIDTHS = {"mods.width.thin", "mods.width.normal", "mods.width.thick"};
	private static final float[] WIDTH_VALUES = {1f, 2f, 3f};
	/** Motion blur strengths, 1 to 4. */
	private static final String[] BLUR = {"mods.blur.low", "mods.blur.medium", "mods.blur.high", "mods.blur.max"};

	private ModCatalog() {}

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
		if (p.supportsNativeCosmeticsPreview()) {
			out.add(new Mod("bedrock-cosmetics", Lang.t("mods.bedrock_cosmetics"), Lang.t("mods.experimental_skins_capes_wings_and_emotes_in"), "looks", Category.SOCIAL,
					"bedrock native skin cape elytra wings emote experimental")
					.toggle(() -> c.bedrockNativeCosmetics, on -> c.bedrockNativeCosmetics = on)
					.settings((host, f) -> {
						f.note(Lang.t("mods.experimental_rendering_full_java_parity_is"));
						f.note(Lang.t("mods.choose_your_skins_capes_and_emotes_in_arctic"));
						f.section(Lang.t("mods.animation"));
						f.toggle(Lang.t("mods.animate_capes"), Lang.t("mods.play_animated_cape_frames_on_your_screen"), () -> !c.reduceCapeMotion, on -> c.reduceCapeMotion = !on);
						f.key("Emote wheel key", "Choose and play one of your emotes", Form.key(() -> c.emoteKey, k -> c.emoteKey = k));
						f.toggle(Lang.t("mods.hold_the_emote_wheel_key"), Lang.t("mods.let_go_to_play_the_selected_emote"), () -> c.emoteWheelHold, on -> c.emoteWheelHold = on);
					}));
		}
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
						if ("stopwatch".equals(w.id)) {
							f.section(Lang.t("mods.key"));
							f.key("Stopwatch key", "Starts, stops and clears it", Form.key(() -> c.stopwatchKey, k -> c.stopwatchKey = k));
						}
						HudLookForm.build(host, f, w);
					});
			m.widget = w;
			out.add(m);
		}
		return out;
	}

	private static void gameplay(List<Mod> out, final ClientConfig c) {
		out.add(new Mod("zoom", Lang.t("mods.zoom"), Lang.t("mods.hold_a_key_to_zoom_in_scroll_to_zoom_further"), "zoom", Category.GAMEPLAY, "optifine spyglass") // i18n-ignore
				.toggle(() -> c.zoomEnabled, on -> c.zoomEnabled = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key("Zoom key", "Hold it to zoom; scroll while zooming to go further", Form.key(() -> c.zoomKey, k -> c.zoomKey = k));
					f.section(Lang.t("mods.feel"));
					f.toggle(Lang.t("mods.smooth_camera_while_zoomed"), Lang.t("mods.like_optifines_zoom_the_view_glides_instead"),
							() -> c.zoomSmoothCamera, on -> c.zoomSmoothCamera = on);
					f.stepper(Lang.t("mods.zoom_sensitivity"), () -> c.zoomSensitivity, v -> c.zoomSensitivity = (int) v, 10, 200, 5, "%.0f%%");
				}));
		out.add(new Mod("emotes", Lang.t("mods.emotes"), Lang.t("mods.hold_a_key_point_at_an_emote_let_go_others"), "looks", Category.SOCIAL,
				"emote wheel dance wave key rebind")
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key("Emote wheel key", "Hold it, move the mouse to an emote, let go",
							Form.key(() -> c.emoteKey, k -> c.emoteKey = k));
					f.note(Lang.t("mods.pick_your_look_and_emotes_in_arctic_launcher"));
				}));
		out.add(new Mod("freelook", Lang.t("mods.freelook"), Lang.t("mods.hold_a_key_to_look_around_without_turning"), "freelook", Category.GAMEPLAY, "perspective 360")
				.toggle(() -> c.freelookEnabled, on -> c.freelookEnabled = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key("Freelook key", "Hold it to look around", Form.key(() -> c.freelookKey, k -> c.freelookKey = k));
					f.note(Lang.t("mods.some_servers_like_hypixel_dont_allow_it_it"));
				}));
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
		out.add(new Mod("pingtags", Lang.t("mods.ping_tags"), Lang.t("mods.other_players_ping_in_front_of_their_name"), "tiers", Category.SOCIAL,
				"ping latency nametag lag ms")
				.toggle(() -> c.pingNametags, on -> c.pingNametags = on));
		out.add(new Mod("durability", Lang.t("mods.low_durability"), Lang.t("mods.a_pop_up_when_armor_or_your_tool_is_about_to"), "durability",
				Category.GAMEPLAY, "armor tool break warning")
				.toggle(() -> c.durabilityWarning, on -> c.durabilityWarning = on));
		out.add(new Mod("controller", Lang.t("mods.controller"), Lang.t("mods.play_with_a_gamepad_xbox_playstation_and"), "controller",
				Category.GAMEPLAY, "gamepad joystick xbox playstation console")
				.toggle(() -> c.controller.enabled, on -> c.controller.enabled = on)
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
		out.add(new Mod("fullbright", Lang.t("mods.fullbright"), Lang.t("mods.see_in_the_dark"), "fullbright", Category.VISUAL, "gamma brightness night vision")
				.toggle(() -> c.fullbright, on -> c.fullbright = on)
				.settings((host, f) -> {
					f.section(Lang.t("mods.key"));
					f.key("Fullbright key", "Switches it on and off", Form.key(() -> c.fullbrightKey, k -> c.fullbrightKey = k));
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
		if (p.motionBlurWorks()) {
			out.add(new Mod("motionblur", Lang.t("mods.motion_blur"), Lang.t("mods.a_soft_blur_as_you_move_and_turn"), "motionblur", Category.VISUAL, "smooth trail") // i18n-ignore
					.toggle(() -> c.motionBlur, on -> c.motionBlur = on)
					.settings((host, f) -> {
						f.section(Lang.t("mods.blur"));
						f.choice(Lang.t("mods.strength"), Lang.all(BLUR), () -> Math.max(0, Math.min(BLUR.length - 1, c.motionBlurStrength - 1)),
								i -> c.motionBlurStrength = i + 1);
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

	private static void chat(List<Mod> out, final ClientConfig c) {
		out.add(new Mod("chat", Lang.t("mods.chat"), Lang.t("mods.timestamps_stacked_lines_mentions"), "chat", Category.CHAT,
				"timestamp stack mention ping")
				.settings((host, f) -> {
					f.section(Lang.t("mods.chat"));
					f.toggle(Lang.t("mods.timestamps"), Lang.t("mods.the_time_in_front_of_every_line"), () -> c.chatTimestamps, on -> c.chatTimestamps = on);
					f.toggle(Lang.t("mods.stack_repeated_lines"), Lang.t("mods.show_x3_instead_of_the_same_line_again"), () -> c.chatStack, on -> c.chatStack = on);
					f.toggle(Lang.t("mods.mentions"), Lang.t("mods.highlight_lines_that_say_your_name_with_a"), () -> c.chatMentions,
							on -> c.chatMentions = on);
					f.toggle(Lang.t("mods.placeholders"), Lang.t("mods.type_and_it_sends_your_position"), () -> c.chatPlaceholders,
							on -> c.chatPlaceholders = on);
					f.note(Lang.t("mods.also") + QuickMessages.PLACEHOLDERS);
				}));
		out.add(new Mod("chathistory", Lang.t("mods.chat_history"), Lang.t("mods.search_every_line_filters_a_second_chat_and"), "search",
				Category.CHAT, "search find filter log scrollback unlimited keyword second chat")
				.page(ChatTab::new));
		out.add(new Mod("emoji", Lang.t("mods.emoji_and_symbols_2"), Lang.t("mods.hearts_stars_and_arrows_pick_one_or_type"), "chat", Category.CHAT,
				"unicode emoji symbols heart star shortcode")
				.toggle(() -> c.chatEmoji, on -> c.chatEmoji = on)
				.page(EmojiTab::new));
		out.add(new Mod("messages", Lang.t("mods.quick_messages"), Lang.t("mods.keys_that_send_a_line_or_a_command"), "messages", Category.CHAT, "macro bind") // i18n-ignore
				.settings(ChatForms::quickMessages));
		out.add(new Mod("autogg", Lang.t("mods.auto_gg"), Lang.t("mods.say_gg_when_a_minigame_ends"), "autogg", Category.CHAT, "good game hypixel")
				.toggle(() -> c.autoGg, on -> c.autoGg = on)
				.settings(ChatForms::autoGg));
		out.add(new Mod("streamer", Lang.t("mods.streamer_mode"), Lang.t("mods.hide_your_name_skin_and_the_server_on_screen"), "streamer", Category.CHAT,
				"privacy snipe hide name")
				.toggle(() -> c.streamerMode, on -> c.streamerMode = on)
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
		f.note(ArcticClient.controller().connected() ? Lang.t("mods.a_controller_is_connected")
				: Lang.t("mods.no_controller_found_plug_one_in_on_1_8_9_1"));
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
			case "bossbar": return " boss bar dragon wither";
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
