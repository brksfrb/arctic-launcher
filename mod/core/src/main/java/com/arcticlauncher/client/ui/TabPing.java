package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.gfx.Gfx;

/**
 * The ping number in the player list, in the place of the game's ping bars
 * (right end of the row). Drawn at the font's native size and on whole GUI
 * pixels so the digits are not clipped.
 */
public final class TabPing {
	private TabPing() {}

	public static int color(int ms) {
		return ms < 80 ? 0xFF55FF55 : ms < 160 ? 0xFFFFFF55 : 0xFFFF5555;
	}

	/**
	 * False when the latency is unknown: the caller should leave the game's icon.
	 */
	public static boolean draw(Gfx g, int width, int x, int y, int ms) {
		if (ms < 0) {
			return false;
		}
		String text = String.valueOf(ms);
		g.text(text, Math.round(x + width - 1 - g.textWidth(text)), Math.round(y), color(ms), true);
		return true;
	}
}
