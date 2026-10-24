package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Dynamic FPS: the frame rate limit drops while the game isn't being used, to save the computer's work: in
 * another window, after a while with no input (nothing moved: not the player, not the mouse), and in the
 * game's menus. The game's own limit still applies; this only ever lowers it.
 */
public final class DynamicFps {
	/** Never limit below this (a slower game looks frozen and takes long to wake up). */
	private static final int FLOOR = 5;
	private static long lastActive = System.currentTimeMillis();
	private static double[] lastSeen;
	private static boolean inMenu;

	private DynamicFps() {}

	/** Each game tick: remembers whether anything moved, and whether a menu is open in the game. */
	public static void tick(Platform platform, boolean screenOpen) {
		inMenu = screenOpen && platform.inWorld();
		double[] now = sample(platform);
		if (lastSeen == null || !sameAs(lastSeen, now)) {
			lastActive = System.currentTimeMillis();
		}
		lastSeen = now;
	}

	/** The player's place and view, and the mouse. */
	private static double[] sample(Platform platform) {
		double[] position = platform.position();
		double[] cursor = platform.cursor();
		double[] out = new double[7];
		for (int i = 0; i < 5; i++) {
			out[i] = position != null && position.length > i ? position[i] : 0;
		}
		out[5] = cursor != null && cursor.length > 0 ? cursor[0] : 0;
		out[6] = cursor != null && cursor.length > 1 ? cursor[1] : 0;
		return out;
	}

	private static boolean sameAs(double[] a, double[] b) {
		for (int i = 0; i < a.length; i++) {
			if (Math.abs(a[i] - b[i]) > 1e-6) {
				return false;
			}
		}
		return true;
	}

	/** The limit to use now when the game's own is {@code current} (0 or less means no limit). */
	public static int limit(int current) {
		ClientConfig config = ArcticClient.config();
		Platform platform = ArcticClient.platform();
		if (config == null || platform == null || !config.dynamicFps) {
			return current;
		}
		return apply(current, platform.windowFocused(), inMenu, System.currentTimeMillis() - lastActive, config);
	}

	/** The rules, apart from the game: the lowest of the limits that apply, never above {@code current}. */
	static int apply(int current, boolean focused, boolean menuOpen, long idleMs, ClientConfig config) {
		int cap = current > 0 ? current : Integer.MAX_VALUE;
		if (!focused) {
			cap = lower(cap, config.dynFpsUnfocused);
		} else {
			if (config.dynFpsIdle > 0 && idleMs >= config.dynFpsIdleSeconds * 1000L) {
				cap = lower(cap, config.dynFpsIdle);
			}
			if (menuOpen) {
				cap = lower(cap, config.dynFpsMenu);
			}
		}
		return cap == Integer.MAX_VALUE ? current : cap;
	}

	/** {@code cap} lowered to {@code wanted} (a positive number; 0 is "no limit of this kind"). */
	private static int lower(int cap, int wanted) {
		return wanted > 0 ? Math.min(cap, Math.max(FLOOR, wanted)) : cap;
	}
}
