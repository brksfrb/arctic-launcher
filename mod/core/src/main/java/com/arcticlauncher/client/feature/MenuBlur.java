package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Menu Blur: the blur of the world behind menus. Where the game has its own (1.20.5 and newer, the
 * "Menu Background Blurriness" video setting, 0 to 10) Arctic's strength replaces it while the feature is on.
 * Older games (1.8.9 to 1.20.4) have no blur, so there the dark gradient the game draws behind menus becomes
 * a frosted veil of the same strength ({@link #frost}); it costs nothing per frame.
 */
public final class MenuBlur {
	public static final int MAX = 10;

	private MenuBlur() {}

	/** The strength the game should use: Arctic's while on, else the game's own {@code vanilla}. */
	public static int strength(int vanilla) {
		return strength(ArcticClient.config(), vanilla);
	}

	static int strength(ClientConfig c, int vanilla) {
		if (c == null || !c.menuBlur) {
			return vanilla;
		}
		return Math.max(0, Math.min(MAX, c.menuBlurStrength));
	}

	/** Vanilla's gradient behind menus fades from alpha 0xC0 to 0xD0; the frosted veil keeps that small step. */
	private static final int STEP = 0x10;

	/**
	 * The colour of the veil behind menus on games with no blur: {@code vanilla} (ARGB, the game's dark
	 * gradient end) while the feature is off, else a cool frosted tint whose opacity follows the strength
	 * (0 clear, 10 nearly opaque).
	 */
	public static int frost(int vanilla) {
		return frost(ArcticClient.config(), vanilla);
	}

	static int frost(ClientConfig c, int vanilla) {
		if (c == null || !c.menuBlur) {
			return vanilla;
		}
		int alpha = Math.min(255, strength(c, 0) * 22 + ((vanilla >>> 24) > 0xC8 ? STEP : 0));
		return alpha << 24 | 0x1A2230;
	}
}
