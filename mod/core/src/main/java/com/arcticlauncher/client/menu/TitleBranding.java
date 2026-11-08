package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
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

	private TitleBranding() {}

	public static void draw(Gfx g, int width, int height) {
		int top = Math.max(6, height / 4 - 48);
		float ps = g.pixelScale();
		// The snowflake turns about its center, drawn in screen pixels from a picture made at exactly
		// that size ("asset:icon-N", linear-filtered) so it stays sharp at every angle.
		int bucket = Math.max(1, Math.min(WORDMARK.length, Math.round(ps)));
		int px = ICON * bucket;
		float angle = (System.nanoTime() % TURN_NANOS) * (360f / TURN_NANOS);
		g.push();
		g.scale(1f / ps);
		g.translate(Math.round(width * ps / 2f), Math.round((top + ICON / 2f) * ps));
		g.rotate(angle);
		g.texture("asset:icon-" + bucket, -px / 2, -px / 2, px, px, 0, 0, px, px, px, px);
		g.pop();
		int scale = Math.max(1, Math.min(WORDMARK.length, Math.round(ps)));
		int w = WORDMARK[scale - 1][0], h = WORDMARK[scale - 1][1];
		int wordTop = top + ICON + 6;
		g.push();
		g.scale(1f / ps);
		g.texture("asset:wordmark-" + scale, Math.round(width * ps / 2f - w / 2f), Math.round(wordTop * ps), w, h, 0, 0, w, h, w, h);
		g.pop();
		Draw.centered(g, "Minecraft " + ArcticClient.platform().minecraftVersion(), width / 2,
				wordTop + Math.round(h / ps) + 5, ArcticClient.style().accent, true);
	}
}
