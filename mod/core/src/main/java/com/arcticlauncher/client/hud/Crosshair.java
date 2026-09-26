package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.CrosshairConfig;
import com.arcticlauncher.client.gfx.Gfx;

/** Draws a custom crosshair (cross, dot, circle or cross with a dot). */
public final class Crosshair {
	private static final int OUTLINE = 0xA0000000;

	private Crosshair() {}

	/** Draw centered at (cx, cy). */
	public static void render(Gfx g, CrosshairConfig c, int cx, int cy) {
		if (c.outline) {
			draw(g, c, cx, cy, 1, OUTLINE);
		}
		draw(g, c, cx, cy, 0, c.color);
	}

	/** One pass; {@code grow} widens every shape (for the outline). */
	private static void draw(Gfx g, CrosshairConfig c, int cx, int cy, int grow, int color) {
		int t = Math.max(1, c.thickness);
		int lo = -(t / 2);
		int hi = lo + t;
		String style = c.style;
		if ("cross".equals(style) || "cross-dot".equals(style)) {
			int near = c.gap;
			int far = c.gap + c.size;
			// Right, left, down, up arms.
			rect(g, cx + near - grow, cy + lo - grow, cx + far + grow, cy + hi + grow, color);
			rect(g, cx - far - grow, cy + lo - grow, cx - near + grow, cy + hi + grow, color);
			rect(g, cx + lo - grow, cy + near - grow, cx + hi + grow, cy + far + grow, color);
			rect(g, cx + lo - grow, cy - far - grow, cx + hi + grow, cy - near + grow, color);
		}
		if ("dot".equals(style) || "cross-dot".equals(style)) {
			int r = Math.max(1, t);
			rect(g, cx - r / 2 - grow, cy - r / 2 - grow, cx - r / 2 + r + grow, cy - r / 2 + r + grow, color);
		}
		if ("circle".equals(style)) {
			ring(g, cx, cy, c.gap + c.size + grow, Math.max(0, c.gap + c.size - t - grow), color);
		}
	}

	private static void rect(Gfx g, int x0, int y0, int x1, int y1, int color) {
		if (x1 > x0 && y1 > y0) {
			g.fill(x0, y0, x1, y1, color);
		}
	}

	/** A pixel ring between two radii. */
	private static void ring(Gfx g, int cx, int cy, int outer, int inner, int color) {
		for (int dy = -outer; dy < outer; dy++) {
			double yc = dy + 0.5;
			int o = (int) Math.round(Math.sqrt(Math.max(0, outer * outer - yc * yc)));
			int i = inner * inner > yc * yc ? (int) Math.round(Math.sqrt(inner * inner - yc * yc)) : 0;
			if (i == 0) {
				rect(g, cx - o, cy + dy, cx + o, cy + dy + 1, color);
			} else {
				rect(g, cx - o, cy + dy, cx - i, cy + dy + 1, color);
				rect(g, cx + i, cy + dy, cx + o, cy + dy + 1, color);
			}
		}
	}
}
