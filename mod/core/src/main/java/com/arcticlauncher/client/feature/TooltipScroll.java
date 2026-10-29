package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Scrollable Tooltips: a tooltip taller than the screen (a long lore) moves with the mouse wheel, so every line can
 * be read. Shorter tooltips are drawn as always. The version hooks ask {@link #shift} when they draw a tooltip and
 * pass the wheel to {@link #wheel}.
 */
public final class TooltipScroll {
	private static final int STEP = 20;
	private static final int MARGIN = 12;
	/** A tooltip not drawn for this long (ms) starts at its top again. */
	private static final long FORGET_MS = 250;
	private static int offset;
	private static int max;
	private static long seen;

	private TooltipScroll() {}

	/** How far down to draw a tooltip {@code tipHeight} tall on a screen {@code screenHeight} tall (0: as the game does). */
	public static int shift(int tipHeight, int screenHeight) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.tooltipScroll) {
			return 0;
		}
		long now = System.currentTimeMillis();
		if (now - seen > FORGET_MS) {
			offset = 0;
		}
		max = Math.max(0, tipHeight - screenHeight + MARGIN);
		if (max == 0) {
			offset = 0;
			return 0;
		}
		seen = now;
		offset = Math.min(offset, max);
		return offset;
	}

	/** The wheel turned (positive: up). True when it moved a tall tooltip, so the game leaves it alone. */
	public static boolean wheel(double amount) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.tooltipScroll || max == 0 || amount == 0 || System.currentTimeMillis() - seen > FORGET_MS) {
			return false;
		}
		offset = Math.max(0, Math.min(max, offset + (amount > 0 ? STEP : -STEP)));
		return true;
	}
}
