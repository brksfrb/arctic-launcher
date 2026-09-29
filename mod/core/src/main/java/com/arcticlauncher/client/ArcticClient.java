package com.arcticlauncher.client;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.Session;
import com.arcticlauncher.client.feature.Features;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.menu.Menus;
import com.arcticlauncher.client.menu.TitleMenu;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;

/**
 * Arctic Client's entry point. Version adapters call {@link #init} once and
 * then forward game events here.
 */
public final class ArcticClient {
	public static final String DEFAULT_SERVER = "https://cosmetics.arcticlauncher.com";
	/** Overrides the Arctic server (for development). */
	public static final String SERVER_PROPERTY = "arctic.cosmetics.url";

	private static Platform platform;
	private static ClientConfig config;
	private static Hud hud;
	private static Looks looks;
	private static Features features;
	private static com.arcticlauncher.client.social.Social social;
	private static com.arcticlauncher.client.voice.VoiceLink voice;
	private static com.arcticlauncher.client.waypoints.Waypoints waypoints;
	private static com.arcticlauncher.client.together.Duel duel;
	private static com.arcticlauncher.client.packs.PackIcons packIcons;
	private static com.arcticlauncher.client.feature.ScreenshotCopy screenshotCopy;
	private static com.arcticlauncher.client.packs.PackBrowser packs;
	/** A duel request from the launcher counts for this long after launching. */
	private static final long DUEL_REQUEST_SECONDS = 180;
	private static boolean waypointKeyWasDown;
	/** The hit color last sent to the game (MIN_VALUE: not yet). */
	private static int appliedHitColor = Integer.MIN_VALUE;

	private ArcticClient() {}

	private static com.arcticlauncher.client.account.AccountSwitcher accounts;
	private static com.arcticlauncher.client.looks.LauncherSkins launcherSkins;

	public static void init(Platform p) {
		com.arcticlauncher.client.looks.OsTrust.installDefault();
		platform = p;
		com.arcticlauncher.client.config.RunningLock.hold(p.configDir());
		config = ClientConfig.load(p.configDir());
		Session session = Session.read(p.configDir());
		applyLauncherStyle(session);
		applyLauncherProxy(session);
		waypoints = com.arcticlauncher.client.waypoints.Waypoints.load(p.configDir());
		hud = new Hud(config);
		features = new Features(p, config);
		looks = new Looks(p, config, serverUrl(session), session.token);
		looks.shareServer(session.shareServer);
		looks.start();
		social = new com.arcticlauncher.client.social.Social(p, looks);
		social.start();
		accounts = new com.arcticlauncher.client.account.AccountSwitcher(p, session.bridgePort, session.bridgeSecret);
		launcherSkins = new com.arcticlauncher.client.looks.LauncherSkins(p, session.bridgePort, session.bridgeSecret);
		voice = new com.arcticlauncher.client.voice.VoiceLink(p, session.bridgePort, session.bridgeSecret);
		voice.start();
		packs = new com.arcticlauncher.client.packs.PackBrowser(p, p.configDir());
		packIcons = new com.arcticlauncher.client.packs.PackIcons(p, session.bridgePort, session.bridgeSecret);
		screenshotCopy = new com.arcticlauncher.client.feature.ScreenshotCopy(session.bridgePort, session.bridgeSecret);
		com.arcticlauncher.client.replay.Replays.init(p, session.bridgePort, session.bridgeSecret);
		com.arcticlauncher.client.net.ProxySelfTest.maybeStart(p);
		duel = new com.arcticlauncher.client.together.Duel(p,
				new com.arcticlauncher.client.together.TogetherLink(session.bridgePort, session.bridgeSecret), social);
		// Asked for in the launcher just now (not on a later restart).
		long age = System.currentTimeMillis() / 1000 - session.duelAt;
		if (session.duelKit != null && age >= 0 && age < DUEL_REQUEST_SECONDS) {
			duel.startWhenReady(session.duelKit, session.duelFriend, session.duelFriendName);
		}
		long replayAge = System.currentTimeMillis() / 1000 - session.replayAt;
		if (session.replayFile != null && replayAge >= 0 && replayAge < DUEL_REQUEST_SECONDS) {
			com.arcticlauncher.client.replay.Replays.watchWhenReady(new java.io.File(session.replayFile));
		}
		p.log(false, "Arctic Client ready: style " + config.style + ", looks from " + looks.baseUrl());
	}

	/** A style (and Fancy) picked in the launcher wins once, until changed there again. */
	private static void applyLauncherStyle(Session session) {
		if (session.style != null && session.styleSet > config.styleFromLauncher) {
			config.style = Style.byId(session.style).id;
			if (session.fancy != null) {
				config.fancy = session.fancy;
			}
			config.styleFromLauncher = session.styleSet;
			saveConfig();
		}
	}

	/** Like the style: a proxy set in the launcher wins once, until changed there again. */
	private static void applyLauncherProxy(Session session) {
		if (session.proxy != null && session.proxySet > config.proxyFromLauncher) {
			config.proxy = session.proxy;
			config.proxyFromLauncher = session.proxySet;
			saveConfig();
		}
		proxyChanged();
	}

	/** The proxy settings changed: apply them to Arctic's and the game's traffic. */
	public static void proxyChanged() {
		com.arcticlauncher.client.looks.Http.useProxy(config.proxy.usable() ? config.proxy : null);
		platform.proxyChanged(config.proxy.usable() ? config.proxy : null);
	}

	private static String serverUrl(Session session) {
		String override = System.getProperty(SERVER_PROPERTY);
		if (override != null) {
			return override;
		}
		return session.url != null ? session.url : DEFAULT_SERVER;
	}

	public static Platform platform() {
		return platform;
	}

	/** Your launcher's skin library, to pick a skin in game. */
	public static com.arcticlauncher.client.looks.LauncherSkins launcherSkins() {
		return launcherSkins;
	}

	/** Switching accounts in game (through the launcher). */
	public static com.arcticlauncher.client.account.AccountSwitcher accounts() {
		return accounts;
	}

	public static ClientConfig config() {
		return config;
	}

	public static void saveConfig() {
		String error = config.save();
		if (error != null) {
			platform.log(true, "Could not save Arctic settings: " + error);
		}
	}

	public static Hud hud() {
		return hud;
	}

	public static Looks looks() {
		return looks;
	}

	public static com.arcticlauncher.client.voice.VoiceLink voice() {
		return voice;
	}

	public static com.arcticlauncher.client.social.Social social() {
		return social;
	}

	public static Features features() {
		return features;
	}

	public static Style style() {
		return Style.byId(config.style);
	}

	/** Restyle vanilla widgets and replace the title screen. */
	public static boolean restyles() {
		return style().restyles;
	}

	// ---- Hooks for adapters ----------------------------------------------------

	/** Draw the HUD over the game (skip while F1 or F3 is up). */
	public static void renderHud(Gfx g) {
		com.arcticlauncher.client.gfx.Draw.fancy = config.fancy;
		com.arcticlauncher.client.replay.ReplayViewer replay = com.arcticlauncher.client.replay.Replays.viewer();
		if (replay != null) {
			// A replay shows its own controls, not your HUD.
			com.arcticlauncher.client.replay.ReplayHud.render(g, style(), replay, platform.hudHidden());
			if (replay.exporting() == null) {
				// Pop-ups would end up in the video.
				com.arcticlauncher.client.notice.Notices.render(g, style());
			}
			return;
		}
		if (platform.inWorld() && !platform.hudHidden()) {
			if (config.waypointsInWorld) {
				com.arcticlauncher.client.waypoints.WorldMarkers.render(g, style(), platform, waypoints.here(platform));
			}
			hud.render(g, style(), false);
		}
		if (platform.inWorld()) {
			if (!platform.hudHidden()) {
				drawVoice(g);
			}
			com.arcticlauncher.client.notice.Notices.render(g, style());
		}
	}

	/** Who's talking (and whether you are), at the left edge. */
	private static void drawVoice(Gfx g) {
		if (voice == null || !voice.active()) {
			return;
		}
		Style s = style();
		int y = g.height() / 2 - 20;
		if (voice.sending()) {
			com.arcticlauncher.client.gfx.Draw.round(g, 4, y, 64, y + 12, 3, 0xC0000000 | (s.panel & 0xFFFFFF));
			g.text("● Talking", 8, y + 2, 0xFF86EFAC, false);
			y += 14;
		}
		for (String name : voice.speakingNames()) {
			int w = g.textWidth(name) + 18;
			com.arcticlauncher.client.gfx.Draw.round(g, 4, y, 4 + w, y + 12, 3, 0xC0000000 | (s.panel & 0xFFFFFF));
			g.text("♪ " + name, 8, y + 2, s.text, false);
			y += 14;
		}
	}

	/** Draw the custom crosshair; false to let the game draw its own. */
	public static boolean renderCrosshair(Gfx g) {
		com.arcticlauncher.client.replay.ReplayViewer replay = com.arcticlauncher.client.replay.Replays.viewer();
		if (replay != null && replay.mode() != com.arcticlauncher.client.replay.ReplayViewer.Mode.FIRST_PERSON) {
			// Watching from a camera: no aim to show.
			return true;
		}
		if (!config.crosshair.enabled || !platform.hasFeatures()) {
			return false;
		}
		com.arcticlauncher.client.hud.Crosshair.render(g, config.crosshair, g.width() / 2, g.height() / 2);
		return true;
	}

	private static final com.arcticlauncher.client.feature.QuickMessages QUICK_MESSAGES =
			new com.arcticlauncher.client.feature.QuickMessages();
	private static final com.arcticlauncher.client.feature.DeathWatch DEATHS = new com.arcticlauncher.client.feature.DeathWatch();
	private static final com.arcticlauncher.client.feature.DurabilityWatch DURABILITY = new com.arcticlauncher.client.feature.DurabilityWatch();
	private static final com.arcticlauncher.client.feature.AutoGg AUTO_GG = new com.arcticlauncher.client.feature.AutoGg();

	/** A chat line arrived (for Auto GG). */
	public static void chatLine(String text) {
		AUTO_GG.onChat(text, config.autoGg, config.autoGgMessage);
	}

	private static final com.arcticlauncher.client.feature.Stopwatch STOPWATCH = new com.arcticlauncher.client.feature.Stopwatch();

	public static com.arcticlauncher.client.feature.Stopwatch stopwatch() {
		return STOPWATCH;
	}

	public static com.arcticlauncher.client.waypoints.Waypoints waypoints() {
		return waypoints;
	}

	/** The waypoint key drops one where you stand. */
	private static void dropWaypoint(boolean down) {
		if (down && !waypointKeyWasDown) {
			com.arcticlauncher.client.waypoints.Waypoint w = waypoints.add(platform, null);
			if (w != null) {
				com.arcticlauncher.client.notice.Notices.post(w.name + " added", w.x + " " + w.y + " " + w.z);
			}
		}
		waypointKeyWasDown = down;
	}

	/** Where you last died this session ("X Y Z"), or null. */
	public static String lastDeath() {
		return DEATHS.lastDeath();
	}

	/** Every client tick (20 a second). */
	public static com.arcticlauncher.client.feature.ScreenshotCopy screenshotCopy() {
		return screenshotCopy;
	}

	public static com.arcticlauncher.client.packs.PackIcons packIcons() {
		return packIcons;
	}

	public static com.arcticlauncher.client.packs.PackBrowser packs() {
		return packs;
	}

	public static com.arcticlauncher.client.together.Duel duel() {
		return duel;
	}

	public static void tick(boolean screenOpen) {
		com.arcticlauncher.client.replay.Replays.tick(screenOpen);
		if (com.arcticlauncher.client.replay.Replays.watching()) {
			return;
		}
		if (duel != null) {
			duel.tick();
		}
		if (config.hitColor != appliedHitColor && platform.hitColorWorks()) {
			appliedHitColor = config.hitColor;
			platform.setHitColor(config.hitColor);
		}
		if (voice != null) {
			voice.tick(config.voiceKey, screenOpen);
		}
		if (platform.inWorld()) {
			features.tick(screenOpen);
			stopEmoteWhenMoving();
			if (platform.hasFeatures()) {
				QUICK_MESSAGES.tick(platform, config.quickMessages, screenOpen);
			}
			if (DEATHS.tick(platform, config.deathNotice) && config.deathNotice) {
				waypoints.death(platform);
			}
			dropWaypoint(!screenOpen && platform.isKeyDown(config.waypointKey));
			if (config.durabilityWarning) {
				DURABILITY.tick(platform);
			}
			AUTO_GG.tick(platform);
			STOPWATCH.tick(!screenOpen && platform.isKeyDown(config.stopwatchKey));
		}
	}

	/** Walking, jumping or sneaking ends your emote. */
	private static void stopEmoteWhenMoving() {
		java.util.UUID me = platform.worldPlayerId();
		if (me != null && looks.cosmetics().playingFor(me) != null && platform.localMoving()) {
			looks.playEmote(null);
		}
	}

	/** A mouse button went down while playing (no screen open). */
	public static void mousePressed(int button) {
		hud.cps().click(button);
	}

	/** A key went down with no screen open; true if Arctic used it. */
	public static boolean keyPressed(int key) {
		if (com.arcticlauncher.client.replay.Replays.watching()) {
			if (key == Keys.RIGHT_SHIFT) {
				return com.arcticlauncher.client.replay.Replays.key(Keys.ESCAPE);
			}
			return com.arcticlauncher.client.replay.Replays.key(key);
		}
		if (key == Keys.RIGHT_SHIFT && platform.inWorld()) {
			platform.openPage(Menus.home());
			return true;
		}
		return false;
	}

	/**
	 * A key went down over one of Minecraft's own screens (not while typing);
	 * true if Arctic used it. Right Shift opens the Arctic menu anywhere.
	 */
	public static boolean screenKeyPressed(int key) {
		if (key == Keys.RIGHT_SHIFT) {
			platform.openPage(Menus.home());
			return true;
		}
		return false;
	}

	/** The page that replaces Minecraft's title screen. */
	public static Page titleMenu() {
		return new TitleMenu();
	}

	public static Page arcticMenu() {
		return Menus.home();
	}
}
