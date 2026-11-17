package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Rideable HUD: on a horse, donkey, mule, camel, strider and the like the game swaps your hunger bar for the
 * mount's health (and the XP bar for the jump bar on jumping mounts). This keeps your hunger and XP too.
 *
 * <p>Layout: the game's own mount health and jump bar stay untouched. Hunger sits on the row right above the
 * mount's hearts (each 10 hearts is a row, so a 15-heart horse has two), right-aligned like the real hunger bar.
 * That is the row the air bubbles use, so under water they overlap: that is why this is off by default. On
 * jumping mounts a slim XP bar (half the width, the game's colours) sits above the hunger row with the level
 * to its left. Mounts without a jump bar (strider, pig) keep the game's XP bar, so only hunger is added.
 */
public final class RideableHud {
	private static final int RIGHT = 91;
	private static final int STEP = 8;
	private static final int ICON = 9;
	private static final int ABOVE_BOTTOM = 39;
	private static final int ROW = 10;
	private static final int XP_WIDTH = 91;
	private static final int XP_HEIGHT = 5;
	private static final int SILHOUETTE = 0xFF2B1E18;
	private static final int MEAT = 0xFFC4723F;
	private static final int XP_GREEN = 0xFF80FF20;

	private RideableHud() {}

	/** Whether to add the hunger bar: the setting is on and the game hides it for this mount. */
	public static boolean showHunger(boolean enabled, float[] bars) {
		return enabled && bars != null && bars.length >= 4;
	}

	/** Whether to add the XP bar: as hunger, and the game also swapped the XP bar for the jump bar. */
	public static boolean showXp(boolean enabled, float[] bars) {
		return showHunger(enabled, bars) && bars[1] >= 0.5f;
	}

	/** Rows of hearts the game draws for a mount (10 per row, 2 health per heart). */
	public static int mountRows(float maxHealth) {
		int hearts = (int) Math.ceil(Math.max(0f, maxHealth) / 2f);
		return Math.max(1, Math.min(5, (hearts + 9) / 10));
	}

	/** Pixels above the screen's bottom where the hunger icons start: just over the mount's heart rows. */
	public static int hungerAbove(float maxHealth) {
		return ABOVE_BOTTOM + (mountRows(maxHealth) - 1) * ROW + ROW;
	}

	public static void render(Gfx g, Platform platform, boolean saturation) {
		float[] bars = platform.mountedBars();
		if (!showHunger(true, bars)) {
			return;
		}
		float[] food = platform.food();
		if (food == null || food.length < 1) {
			return;
		}
		int top = g.height() - hungerAbove(bars[0]);
		int right = g.width() / 2 + RIGHT;
		float level = Math.max(0f, Math.min(20f, food[0]));
		for (int i = 0; i < 10; i++) {
			int left = right - i * STEP - ICON;
			SaturationBar.overlay(g, left, top, SaturationBar.OUTLINE, SILHOUETTE);
			float have = Math.max(0f, Math.min(1f, (level - i * 2f) / 2f));
			if (have > 0f) {
				SaturationBar.overlay(g, left, top, have > 0.5f ? SaturationBar.WHOLE : SaturationBar.HALF, MEAT);
			}
		}
		if (saturation) {
			SaturationBar.renderAt(g, platform, top);
		}
		if (showXp(true, bars)) {
			renderXp(g, bars[2], (int) bars[3], top - XP_HEIGHT - 3, right);
		}
	}

	private static void renderXp(Gfx g, float progress, int level, int top, int right) {
		int left = right - XP_WIDTH;
		g.fill(left - 1, top - 1, right + 1, top + XP_HEIGHT + 1, 0xFF000000);
		g.fill(left, top, right, top + XP_HEIGHT, 0xFF3A3A3A);
		int filled = Math.round(XP_WIDTH * Math.max(0f, Math.min(1f, progress)));
		if (filled > 0) {
			g.fill(left, top, left + filled, top + XP_HEIGHT, XP_GREEN);
		}
		if (level > 0) {
			String text = Integer.toString(level);
			g.text(text, left - 4 - g.textWidth(text), top - 1, XP_GREEN, true);
		}
	}
}
