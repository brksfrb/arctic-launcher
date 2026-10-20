package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Saturation, the hidden half of hunger, drawn on the game's own hunger bar (like AppleSkin): a gold line along the
 * bottom of each food icon, as full as that icon's share of your saturation. Hunger only drops once saturation
 * is gone, so a bar with gold under it means a while longer before the icons start to empty.
 */
public final class SaturationBar {
	/** The hunger bar's icons sit 8 pixels apart, 9 wide, starting 91 pixels right of the screen's middle. */
	private static final int STEP = 8;
	private static final int ICON = 9;
	private static final int RIGHT = 91;
	private static final int ABOVE_BOTTOM = 39;
	private static final int GOLD = 0xFFFFC22E;
	private static final int LINE = 2;

	private SaturationBar() {}

	public static void render(Gfx g, Platform platform) {
		float[] food = platform.food();
		if (food == null || food.length < 2 || !platform.hungerBarShown()) {
			return;
		}
		float saturation = Math.max(0f, Math.min(food[1], food[0]));
		if (saturation <= 0f) {
			return;
		}
		int right = g.width() / 2 + RIGHT;
		int top = g.height() - ABOVE_BOTTOM;
		for (int i = 0; i < 10; i++) {
			// Icon 0 is the rightmost; each icon stands for two hunger points.
			float share = Math.max(0f, Math.min(1f, (saturation - i * 2f) / 2f));
			if (share <= 0f) {
				break;
			}
			int left = right - i * STEP - ICON;
			int filled = Math.max(1, Math.round(ICON * share));
			g.fill(left + ICON - filled, top + ICON - LINE, left + ICON, top + ICON, GOLD);
		}
	}
}
