package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.BrandMark;
import com.arcticlauncher.client.gfx.Gfx;

/** Arctic's title artwork, shared by the native menu and the page preview. */
public final class TitleBranding {
	/** The snowflake's size in GUI pixels (1.5 times the old 24). */
	private static final int ICON = 36;
	/** One full turn, in nanoseconds of wall-clock time (not ticks, so it is smooth at any frame rate). */
	private static final long TURN_NANOS = 10_000_000_000L;
	private static final int[][] WORDMARK = {
		{88, 18}, {175, 37}, {263, 55}, {351, 74}, {438, 92}, {526, 111}, {613, 129}, {701, 147},
	};

	private static final String[] WORDMARK_KEYS = {
			"asset:wordmark-1", "asset:wordmark-2", "asset:wordmark-3", "asset:wordmark-4",
			"asset:wordmark-5", "asset:wordmark-6", "asset:wordmark-7", "asset:wordmark-8"
	};
	private static String version;
	private static String subtitle;

	private TitleBranding() {}

	public static void draw(Gfx g, int width, int height) {
		int top = Math.max(6, height / 4 - 48);
		float ps = g.pixelScale();
		float angle = (System.nanoTime() % TURN_NANOS) * (360f / TURN_NANOS);
		BrandMark.drawCentered(g, width / 2f, top + ICON / 2f, ICON, angle);
		int scale = Math.max(1, Math.min(WORDMARK.length, Math.round(ps)));
		int w = WORDMARK[scale - 1][0], h = WORDMARK[scale - 1][1];
		int wordTop = top + ICON + 6;
		g.push();
		g.scale(1f / ps);
		g.texture(WORDMARK_KEYS[scale - 1], Math.round(width * ps / 2f - w / 2f), Math.round(wordTop * ps), w, h, 0, 0, w, h, w, h);
		g.pop();
		String current = ArcticClient.platform().minecraftVersion();
		if (!current.equals(version)) {
			version = current;
			subtitle = "Minecraft " + current;
		}
		boolean menuText = g.menuText(Draw.TEST_MENU_FANCY
				&& ArcticClient.style() == com.arcticlauncher.client.style.Style.ARCTIC);
		try {
			Draw.centered(g, subtitle, width / 2, wordTop + Math.round(h / ps) + 5, ArcticClient.style().accent, true);
		} finally {
			g.menuText(menuText);
		}
	}
}
