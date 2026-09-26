package com.arcticlauncher.client;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.Session;
import com.arcticlauncher.client.feature.Features;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.menu.ArcticMenu;
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

	private ArcticClient() {}

	public static void init(Platform p) {
		platform = p;
		config = ClientConfig.load(p.configDir());
		Session session = Session.read(p.configDir());
		applyLauncherStyle(session);
		hud = new Hud(config);
		features = new Features(p, config);
		looks = new Looks(p, config, serverUrl(session), session.token);
		looks.start();
		p.log(false, "Arctic Client ready: style " + config.style + ", looks from " + looks.baseUrl());
	}

	/** A style picked in the launcher wins once, until changed there again. */
	private static void applyLauncherStyle(Session session) {
		if (session.style != null && session.styleSet > config.styleFromLauncher) {
			config.style = Style.byId(session.style).id;
			config.styleFromLauncher = session.styleSet;
			saveConfig();
		}
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
		if (platform.inWorld() && !platform.hudHidden()) {
			hud.render(g, style(), false);
		}
	}

	/** Every client tick (20 a second). */
	public static void tick(boolean screenOpen) {
		if (platform.inWorld()) {
			features.tick(screenOpen);
		}
	}

	/** A mouse button went down while playing (no screen open). */
	public static void mousePressed(int button) {
		hud.cps().click(button);
	}

	/** A key went down with no screen open; true if Arctic used it. */
	public static boolean keyPressed(int key) {
		if (key == Keys.RIGHT_SHIFT && platform.inWorld()) {
			platform.openPage(new ArcticMenu());
			return true;
		}
		return false;
	}

	/** The page that replaces Minecraft's title screen. */
	public static Page titleMenu() {
		return new TitleMenu();
	}

	public static Page arcticMenu() {
		return new ArcticMenu();
	}
}
