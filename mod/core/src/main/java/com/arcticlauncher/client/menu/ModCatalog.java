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

/** Every card on the Mods screen, for this version of the game. */
final class ModCatalog {
	private static final int[] OUTLINE_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFF000000};
	private static final int[] HIT_COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF97316, 0xFFE879F9};
	private static final String[] TIMES = {"Day", "Noon", "Sunset", "Night", "Midnight"};
	private static final int[] TIME_TICKS = {1000, 6000, 12500, 13500, 18000};
	private static final String[] WIDTHS = {"Thin", "Normal", "Thick"};
	private static final float[] WIDTH_VALUES = {1f, 2f, 3f};

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
		out.add(hudStyle(c));
		for (final HudWidget w : ArcticClient.hud().widgets()) {
			final HudSlot slot = ArcticClient.hud().slot(w);
			Mod m = new Mod("hud." + w.id, w.name, w.description, w.id, Category.HUD, "hud widget")
					.toggle(() -> slot.enabled, on -> {
						slot.enabled = on;
						if (!on) {
							slot.placed = false;
						}
					})
					.settings((host, f) -> {
						if ("stopwatch".equals(w.id)) {
							f.section("Key");
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
		out.add(new Mod("zoom", "Zoom", "Hold a key to zoom in; scroll to zoom further", "zoom", Category.GAMEPLAY, "optifine spyglass")
				.toggle(() -> c.zoomEnabled, on -> c.zoomEnabled = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Zoom key", "Hold it to zoom; scroll while zooming to go further", Form.key(() -> c.zoomKey, k -> c.zoomKey = k));
					f.section("Feel");
					f.toggle("Smooth camera while zoomed", "Like OptiFine's zoom: the view glides instead of snapping",
							() -> c.zoomSmoothCamera, on -> c.zoomSmoothCamera = on);
				}));
		out.add(new Mod("freelook", "Freelook", "Hold a key to look around without turning", "freelook", Category.GAMEPLAY, "perspective 360")
				.toggle(() -> c.freelookEnabled, on -> c.freelookEnabled = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Freelook key", "Hold it to look around", Form.key(() -> c.freelookKey, k -> c.freelookKey = k));
					f.note("Some servers (like Hypixel) don't allow it; it stays off there.");
				}));
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
			out.add(new Mod("replays", "Replays", "Every session records; keep one, watch it, make videos", "replay", Category.GAMEPLAY,
					"replay record clip video camera recording")
					.toggle(() -> c.replayRecording, on -> c.replayRecording = on)
					.page(ReplaysTab::new));
		}
		out.add(new Mod("death", "Death Position", "A pop-up with where you died, and a Death waypoint", "death", Category.GAMEPLAY, "coords")
				.toggle(() -> c.deathNotice, on -> c.deathNotice = on));
		out.add(new Mod("durability", "Low Durability", "A pop-up when armor or your tool is about to break", "durability",
				Category.GAMEPLAY, "armor tool break warning")
				.toggle(() -> c.durabilityWarning, on -> c.durabilityWarning = on));
		out.add(new Mod("leave", "Confirm Leaving", "Click Disconnect or Save and Quit twice to leave", "leave", Category.GAMEPLAY,
				"quit disconnect")
				.toggle(() -> c.confirmLeave, on -> c.confirmLeave = on));
	}

	private static void visuals(List<Mod> out, final ClientConfig c, Platform p) {
		out.add(new Mod("fullbright", "Fullbright", "See in the dark", "fullbright", Category.VISUAL, "gamma brightness night vision")
				.toggle(() -> c.fullbright, on -> c.fullbright = on)
				.settings((host, f) -> {
					f.section("Key");
					f.key("Fullbright key", "Switches it on and off", Form.key(() -> c.fullbrightKey, k -> c.fullbrightKey = k));
				}));
		out.add(new Mod("crosshair", "Crosshair", "Your own crosshair: shape, size, color", "crosshair", Category.VISUAL, "aim")
				.toggle(() -> c.crosshair.enabled, on -> c.crosshair.enabled = on)
				.page(CrosshairTab::new));
		out.add(new Mod("time", "Time Changer", "Pick the time of day on your screen", "time", Category.VISUAL, "day night sky")
				.toggle(() -> c.timeLock >= 0, on -> c.timeLock = on ? TIME_TICKS[1] : -1)
				.settings((host, f) -> {
					f.section("Time");
					f.choice("Time of day", TIMES, () -> indexOf(TIME_TICKS, c.timeLock), i -> c.timeLock = TIME_TICKS[i]);
					f.note("Only on your screen; the server's time doesn't change.");
				}));
		out.add(new Mod("weather", "Clear Weather", "Hide rain and thunder on your screen", "weather", Category.VISUAL, "rain snow")
				.toggle(() -> c.clearWeather, on -> c.clearWeather = on));
		out.add(new Mod("fire", "Low Fire", "A shorter fire overlay when you're burning", "fire", Category.VISUAL, "burning overlay")
				.toggle(() -> c.lowFire, on -> c.lowFire = on));
		if (p.outlineTweaks()) {
			out.add(new Mod("outline", "Block Outline", "The outline on the block you look at", "outline", Category.VISUAL, "selection")
					.toggle(() -> c.outlineColor != 0, on -> {
						c.outlineColor = on ? OUTLINE_COLORS[0] : 0;
						c.outlineWidth = on ? WIDTH_VALUES[1] : 1f;
					})
					.settings((host, f) -> {
						f.section("Outline");
						f.swatches("Color", OUTLINE_COLORS, () -> c.outlineColor, v -> c.outlineColor = v);
						f.choice("Thickness", WIDTHS, () -> indexOf(WIDTH_VALUES, c.outlineWidth), i -> c.outlineWidth = WIDTH_VALUES[i]);
					}));
		}
		if (p.hitColorWorks()) {
			out.add(new Mod("hitcolor", "Hit Color", "Recolor the red flash players and mobs get when they're hit", "hitcolor", Category.VISUAL,
					"damage red tint")
					.toggle(() -> c.hitColor != 0, on -> c.hitColor = on ? HIT_COLORS[1] & 0xFFFFFF : 0)
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
		out.add(new Mod("packs", "Resource Packs", "Your packs, and new ones from Modrinth", "packs", Category.VISUAL,
				"texture pack modrinth").page(PacksTab::new));
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
		out.add(new Mod("looks", "Looks", "Capes, cosmetics and emotes", "looks", Category.SOCIAL, "cape cosmetic emote")
				.page(LooksTab::new));
		out.add(new Mod("account", "Account", "Switch accounts, or use a proxy", "account", Category.CLIENT, "proxy login")
				.page(AccountPage::new));
		out.add(new Mod("style", "Menu Style", "How the menus look; smooth font", "style", Category.CLIENT, "theme font fancy")
				.page(StyleTab::new));
	}

	/** One look for every HUD widget, with the FPS widget as the preview. */
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
