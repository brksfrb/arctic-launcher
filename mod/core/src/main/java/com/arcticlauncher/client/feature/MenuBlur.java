package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Menu Blur: the blur of the world behind menus. Where the game has its own (1.20.5 and newer, the
 * "Menu Background Blurriness" video setting, 0 to 10) Arctic's strength replaces it while the feature is on.
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
}
