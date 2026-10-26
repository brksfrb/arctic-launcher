package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Saturation, the hidden half of hunger, drawn on the game's own hunger bar like AppleSkin: the food icons
 * that your saturation covers get a golden overlay (a whole icon for 2 points, half an icon for 1), so you
 * see how many icons you could lose before hunger starts to drop. Hunger only drops once saturation is gone.
 * The overlay has the shape of the game's own drumstick, so it sits exactly on the icon.
 */
public final class SaturationBar {
	/** The hunger bar's icons sit 8 pixels apart, 9 wide, starting 91 pixels right of the screen's middle. */
	private static final int STEP = 8;
	private static final int ICON = 9;
	private static final int RIGHT = 91;
	private static final int ABOVE_BOTTOM = 39;
	/** Gold, a little see-through so the meat shows through. */
	private static final int GOLD = 0xCCFFC22E;

	/** The meat of a whole drumstick (the game's food_full), row by row; X is drawn. */
	static final String[] WHOLE = {
		".........",
		"..XX.....",
		".XXXX....",
		".XXXXX...",
		"..XXXX...",
		"...XXX...",
		"......X..",
		".......X.",
		".........",
	};
	/** The meat of half a drumstick (food_half). */
	static final String[] HALF = {
		".........",
		"...X.....",
		"..XXX....",
		"...XXX...",
		"...XXX...",
		"....XX...",
		"......X..",
		".......X.",
		".........",
	};

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
			float share = (saturation - i * 2f) / 2f;
			if (share <= 0f) {
				break;
			}
			int left = right - i * STEP - ICON;
			overlay(g, left, top, share > 0.5f ? WHOLE : HALF);
		}
	}

	/** Paints the mask's X pixels, a run of them at a time. */
	static void overlay(Gfx g, int left, int top, String[] mask) {
		for (int row = 0; row < mask.length; row++) {
			String line = mask[row];
			int start = -1;
			for (int col = 0; col <= line.length(); col++) {
				boolean on = col < line.length() && line.charAt(col) == 'X';
				if (on && start < 0) {
					start = col;
				} else if (!on && start >= 0) {
					g.fill(left + start, top + row, left + col, top + row + 1, GOLD);
					start = -1;
				}
			}
		}
	}
}
