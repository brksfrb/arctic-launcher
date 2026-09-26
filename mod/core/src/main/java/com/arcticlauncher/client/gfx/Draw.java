package com.arcticlauncher.client.gfx;

/** Shapes and text helpers built from {@link Gfx} fills. */
public final class Draw {
	/** Per-row insets for pixel-rounded corners of radius 0..4. */
	private static final int[][] CORNERS = {{}, {1}, {2, 1}, {3, 1, 1}, {4, 2, 1, 1}};

	public static final int LINE = 9;

	private Draw() {}

	private static int[] corner(int radius, int w, int h) {
		int r = Math.max(0, Math.min(radius, Math.min(4, Math.min(w, h) / 2)));
		return CORNERS[r];
	}

	/** Filled rectangle with pixel-rounded corners; rows never overlap. */
	public static void round(Gfx g, int x0, int y0, int x1, int y1, int radius, int color) {
		int[] in = corner(radius, x1 - x0, y1 - y0);
		int r = in.length;
		for (int i = 0; i < r; i++) {
			g.fill(x0 + in[i], y0 + i, x1 - in[i], y0 + i + 1, color);
			g.fill(x0 + in[i], y1 - i - 1, x1 - in[i], y1 - i, color);
		}
		g.fill(x0, y0 + r, x1, y1 - r, color);
	}

	/** One-pixel outline following {@link #round}'s shape. */
	public static void outline(Gfx g, int x0, int y0, int x1, int y1, int radius, int color) {
		int[] in = corner(radius, x1 - x0, y1 - y0);
		int r = in.length;
		int top = r == 0 ? 0 : in[0];
		g.fill(x0 + top, y0, x1 - top, y0 + 1, color);
		g.fill(x0 + top, y1 - 1, x1 - top, y1, color);
		for (int i = 1; i <= r; i++) {
			int inset = i < r ? in[i] : 0;
			int span = Math.max(1, in[i - 1] - inset);
			edgeRow(g, x0, x1, inset, span, y0 + i, color);
			edgeRow(g, x0, x1, inset, span, y1 - i - 1, color);
		}
		int from = y0 + r + 1;
		int to = y1 - r - 1;
		if (r == 0) {
			from = y0 + 1;
			to = y1 - 1;
		}
		if (to > from) {
			g.fill(x0, from, x0 + 1, to, color);
			g.fill(x1 - 1, from, x1, to, color);
		}
	}

	private static void edgeRow(Gfx g, int x0, int x1, int inset, int span, int y, int color) {
		g.fill(x0 + inset, y, x0 + inset + span, y + 1, color);
		g.fill(x1 - inset - span, y, x1 - inset, y + 1, color);
	}

	/** A filled disc (pixel circle). */
	public static void disc(Gfx g, int cx, int cy, int radius, int color) {
		for (int dy = -radius; dy < radius; dy++) {
			double yc = dy + 0.5;
			int half = (int) Math.round(Math.sqrt(Math.max(0, radius * radius - yc * yc)));
			if (half > 0) {
				g.fill(cx - half, cy + dy, cx + half, cy + dy + 1, color);
			}
		}
	}

	public static void centered(Gfx g, String text, int cx, int y, int color, boolean shadow) {
		g.text(text, cx - g.textWidth(text) / 2, y, color, shadow);
	}

	/** Text drawn {@code scale} times larger, top-left at (x, y). */
	public static void big(Gfx g, String text, int x, int y, float scale, int color) {
		g.push();
		g.translate(x, y);
		g.scale(scale);
		g.text(text, 0, 0, color, true);
		g.pop();
	}

	/** Shorten text with "…" to fit {@code maxWidth}. */
	public static String fit(Gfx g, String text, int maxWidth) {
		if (g.textWidth(text) <= maxWidth) {
			return text;
		}
		String dots = "...";
		int end = text.length();
		while (end > 0 && g.textWidth(text.substring(0, end) + dots) > maxWidth) {
			end--;
		}
		return text.substring(0, end) + dots;
	}

	/** Color with its alpha multiplied by {@code a} (0..1). */
	public static int alpha(int color, float a) {
		int alpha = (int) (((color >>> 24) & 0xFF) * Math.max(0f, Math.min(1f, a)));
		return (alpha << 24) | (color & 0xFFFFFF);
	}

	/** Linear blend between two ARGB colors. */
	public static int mix(int from, int to, float t) {
		float k = Math.max(0f, Math.min(1f, t));
		int out = 0;
		for (int shift = 0; shift < 32; shift += 8) {
			int a = (from >>> shift) & 0xFF;
			int b = (to >>> shift) & 0xFF;
			out |= ((int) (a + (b - a) * k) & 0xFF) << shift;
		}
		return out;
	}
}
