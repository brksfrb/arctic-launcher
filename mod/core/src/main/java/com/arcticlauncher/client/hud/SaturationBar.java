package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Saturation, the hidden half of hunger, drawn on the game's own hunger bar like AppleSkin. The food icons
 * your saturation covers get a soft golden glow (a whole icon for 2 points, half for 1): a faint halo round
 * the drumstick and a see-through tint on the meat, so the game's own shading still shows through. Hunger only
 * drops once saturation is gone.
 *
 * <p>With food in your hand, the icons it would fill flash (the hunger it gives) and the saturation it would
 * add flashes in gold, before you eat.
 */
public final class SaturationBar {
	/** The hunger bar's icons sit 8 pixels apart, 9 wide, starting 91 pixels right of the screen's middle. */
	private static final int STEP = 8;
	private static final int ICON = 9;
	private static final int RIGHT = 91;
	private static final int ABOVE_BOTTOM = 39;
	/** Gold without its see-through part; each use adds its own. */
	private static final int GOLD = 0xFFDD00;
	/** The colour of the game's drumstick meat, for the hunger a food in your hand would give. */
	private static final int MEAT = 0xC4723F;
	/** How see-through the glow is (0 to 255). */
	private static final int HALO = 0x38;
	private static final int TINT = 0x66;

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
	/** The whole drumstick with its outline (the game's empty food icon). */
	static final String[] OUTLINE = {
		"..XX.....",
		".XXXX....",
		"XXXXXX...",
		"XXXXXXX..",
		".XXXXXX..",
		"..XXXXX..",
		"...XXXXXX",
		"......XXX",
		"......XX.",
	};

	private SaturationBar() {}

	public static void render(Gfx g, Platform platform) {
		float[] food = platform.food();
		if (food == null || food.length < 2 || !platform.hungerBarShown()) {
			return;
		}
		float level = Math.max(0f, Math.min(20f, food[0]));
		float saturation = Math.max(0f, Math.min(food[1], level));
		float[] held = platform.heldFood();
		int right = g.width() / 2 + RIGHT;
		int top = g.height() - ABOVE_BOTTOM;
		for (int i = 0; i < 10; i++) {
			int left = right - i * STEP - ICON;
			// Each icon stands for two hunger points; icon 0 is the rightmost.
			float have = share(saturation, i);
			if (have > 0f) {
				glow(g, left, top, have);
			}
		}
		if (held == null) {
			return;
		}
		float newLevel = Math.min(20f, level + Math.max(0f, held[0]));
		float newSaturation = Math.min(newLevel, saturation + Math.max(0f, held[1]));
		// Flashes between faint and clear, like AppleSkin's.
		float pulse = 0.35f + 0.65f * (0.5f + 0.5f * (float) Math.sin(System.currentTimeMillis() / 180.0));
		for (int i = 0; i < 10; i++) {
			int left = right - i * STEP - ICON;
			if (share(newLevel, i) > share(level, i)) {
				overlay(g, left, top, share(newLevel, i) > 0.5f ? WHOLE : HALF, color(MEAT, (int) (0xB0 * pulse)));
			}
			if (share(newSaturation, i) > share(saturation, i)) {
				overlay(g, left, top, share(newSaturation, i) > 0.5f ? WHOLE : HALF, color(GOLD, (int) (0xC0 * pulse)));
			}
		}
	}

	/** How much of icon {@code i} (0 to 1) {@code points} fill. */
	private static float share(float points, int i) {
		return Math.max(0f, Math.min(1f, (points - i * 2f) / 2f));
	}

	private static int color(int rgb, int alpha) {
		return Math.max(0, Math.min(255, alpha)) << 24 | rgb;
	}

	/** A faint halo round the drumstick and a see-through tint on the meat. */
	private static void glow(Gfx g, int left, int top, float share) {
		overlay(g, left, top, OUTLINE, color(GOLD, HALO));
		overlay(g, left, top, share > 0.5f ? WHOLE : HALF, color(GOLD, TINT));
	}

	/** Paints the mask's X pixels, a run of them at a time. */
	static void overlay(Gfx g, int left, int top, String[] mask, int color) {
		for (int row = 0; row < mask.length; row++) {
			String line = mask[row];
			int start = -1;
			for (int col = 0; col <= line.length(); col++) {
				boolean on = col < line.length() && line.charAt(col) == 'X';
				if (on && start < 0) {
					start = col;
				} else if (!on && start >= 0) {
					g.fill(left + start, top + row, left + col, top + row + 1, color);
					start = -1;
				}
			}
		}
	}
}
