package com.arcticlauncher.client.gfx;

/**
 * Anti-aliased rounded shapes at the screen's real resolution (for the
 * Fancy style): drawn in physical pixels under a 1/GUI-scale transform,
 * each corner row from the circle, with the edge pixel blended by coverage.
 */
final class Smooth {
	/** Coverage below this is left out (invisible anyway). */
	private static final float MIN_COVERAGE = 0.03f;

	private Smooth() {}

	/** Inset of a rounded corner of radius {@code r} on row {@code i} (from the edge). */
	private static float inset(float r, int i) {
		float dy = r - i - 0.5f;
		return r - (float) Math.sqrt(Math.max(0f, r * r - dy * dy));
	}

	static void round(Gfx g, int x0, int y0, int x1, int y1, float radius, int color) {
		float s = g.pixelScale();
		g.push();
		g.scale(1f / s);
		int ax = Math.round(x0 * s);
		int ay = Math.round(y0 * s);
		int bx = Math.round(x1 * s);
		int by = Math.round(y1 * s);
		float r = Math.min(radius * s, Math.min(bx - ax, by - ay) / 2f);
		// Row by row is ~50 rectangles a panel; the GUI renderer checks each
		// new one against the rest, which shows up with a busy HUD.
		if (g.roundedFill(ax, ay, bx, by, (int) Math.floor(r), color)) {
			g.pop();
			return;
		}
		int rows = (int) Math.ceil(r);
		for (int i = 0; i < rows; i++) {
			span(g, ax, bx, ay + i, inset(r, i), color);
			span(g, ax, bx, by - 1 - i, inset(r, i), color);
		}
		if (by - rows > ay + rows) {
			g.fill(ax, ay + rows, bx, by - rows, color);
		}
		g.pop();
	}

	/** One row from {@code ax + inset} to {@code bx - inset}, with blended ends. */
	private static void span(Gfx g, int ax, int bx, int y, float inset, int color) {
		int full = (int) Math.ceil(inset);
		if (bx - full > ax + full) {
			g.fill(ax + full, y, bx - full, y + 1, color);
		}
		float coverage = full - inset;
		if (coverage > MIN_COVERAGE && full >= 1) {
			int edge = Draw.alpha(color, coverage);
			g.fill(ax + full - 1, y, ax + full, y + 1, edge);
			g.fill(bx - full, y, bx - full + 1, y + 1, edge);
		}
	}

	/** A rounded outline {@code thickness} GUI pixels wide. */
	static void outline(Gfx g, int x0, int y0, int x1, int y1, float radius, int color) {
		float s = g.pixelScale();
		g.push();
		g.scale(1f / s);
		int ax = Math.round(x0 * s);
		int ay = Math.round(y0 * s);
		int bx = Math.round(x1 * s);
		int by = Math.round(y1 * s);
		int t = Math.max(1, Math.round(s));
		float r = Math.min(radius * s, Math.min(bx - ax, by - ay) / 2f);
		int rows = Math.max((int) Math.ceil(r), t);
		float inner = Math.max(0f, r - t);
		for (int i = 0; i < rows; i++) {
			ringRow(g, ax, bx, ay + i, i, r, inner, t, color);
			ringRow(g, ax, bx, by - 1 - i, i, r, inner, t, color);
		}
		if (by - rows > ay + rows) {
			g.fill(ax, ay + rows, ax + t, by - rows, color);
			g.fill(bx - t, ay + rows, bx, by - rows, color);
		}
		g.pop();
	}

	/** Row {@code i} of a ring: between the outer and the inner rounded edge. */
	private static void ringRow(Gfx g, int ax, int bx, int y, int i, float r, float inner, int t, int color) {
		float out = i < Math.ceil(r) ? inset(r, i) : 0f;
		int outFull = (int) Math.ceil(out);
		int in;
		if (i < t) {
			// The top/bottom band: solid across.
			in = (bx - ax) / 2;
		} else {
			int j = i - t;
			in = t + (j < Math.ceil(inner) ? (int) Math.floor(inset(inner, j)) : 0);
		}
		int left0 = ax + outFull;
		int left1 = Math.min(ax + in, (ax + bx) / 2);
		if (left1 > left0) {
			g.fill(left0, y, left1, y + 1, color);
			g.fill(bx - (left1 - ax), y, bx - outFull, y + 1, color);
		}
		float coverage = outFull - out;
		if (coverage > MIN_COVERAGE && outFull >= 1) {
			int edge = Draw.alpha(color, coverage);
			g.fill(left0 - 1, y, left0, y + 1, edge);
			g.fill(bx - outFull, y, bx - outFull + 1, y + 1, edge);
		}
	}

	static void disc(Gfx g, int cx, int cy, float radius, int color) {
		round(g, Math.round(cx - radius), Math.round(cy - radius), Math.round(cx + radius), Math.round(cy + radius), radius, color);
	}
}
