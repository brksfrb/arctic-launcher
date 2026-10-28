package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.gfx.Gfx;

/** A small Arctic logo and the launcher's version, so a player can tell the game runs through Arctic. */
public final class Badge {
	private static final int ICON = 10;
	private static final int GAP = 4;
	private static final int MARGIN = 6;
	private static final int COLOR = 0xCCFFFFFF;

	private Badge() {}

	/** "Arctic 0.5.5"; just "Arctic" when the launcher did not say its version (a dev run). */
	public static String label() {
		String version = System.getProperty("arctic.launcher");
		return version == null || version.isEmpty() ? "Arctic" : "Arctic " + version;
	}

	/** Bottom right of a menu. */
	public static void drawCorner(Gfx g, int width, int height) {
		int textWidth = g.textWidth(label());
		draw(g, width - MARGIN - ICON - GAP - textWidth, height - MARGIN - ICON);
	}

	/** Bottom middle, clear of the title screen's version (left) and copyright (right). */
	public static void drawCentered(Gfx g, int width, int height) {
		int total = ICON + GAP + g.textWidth(label());
		draw(g, (width - total) / 2, height - MARGIN - ICON);
	}

	private static void draw(Gfx g, int x, int y) {
		g.texture("icon", x, y, ICON, ICON, 0, 0, 256, 256, 256, 256);
		g.text(label(), x + ICON + GAP, y + (ICON - 8) / 2, COLOR, true);
	}
}
