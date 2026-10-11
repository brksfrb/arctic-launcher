package com.arcticlauncher.client.hud;

import java.util.List;

/**
 * The minimap's picture, made here for every version: the adapter only reads map colors for a
 * block area and uploads pixels. A grid of colors around you is kept (shifted as you move, a
 * stripe refreshed at a time so changes show), and each frame the picture is sampled from it at
 * your exact position: it glides instead of jumping a block at a time, can turn with you, can
 * be round, and carries smooth (anti-aliased) arrows and dots.
 */
final class MinimapView {
	/** The picture's size in pixels (it's shown smaller, so edges stay smooth). */
	static final int PICTURE = 192;
	/** Blocks kept around you: enough for the widest zoom, turned to any angle. */
	static final int GRID = 192;
	/** Grid rows read again per refresh, and how often (a full pass about every second). */
	private static final int STRIPE = 24;
	private static final long STRIPE_NANOS = 125_000_000L;
	/** The grid moves once you're this far from its middle. */
	private static final int RECENTER = 16;
	static final int UNLOADED = 0xFF101418;

	/** Reads the map colors (ARGB, 0 = not loaded) of the w×h columns from (x0, z0), row by row. */
	interface Columns {
		boolean read(int x0, int z0, int w, int h, int[] out);
	}

	/** Something drawn on the map: where (block x, z), which way (yaw, NaN for a dot), its color. */
	static final class Marker {
		final double x;
		final double z;
		final double yaw;
		final int color;
		final boolean big;

		Marker(double x, double z, double yaw, int color, boolean big) {
			this.x = x;
			this.z = z;
			this.yaw = yaw;
			this.color = color;
			this.big = big;
		}
	}

	private final int[] grid = new int[GRID * GRID];
	private int gridX0 = Integer.MIN_VALUE;
	private int gridZ0;
	private int nextStripe;
	private long stripeAt;
	final int[] pixels = new int[PICTURE * PICTURE];
	private final int[] strip = new int[GRID * STRIPE];

	/** Keep the grid around (px, pz) and fresh. Game thread. */
	void update(Columns columns, double px, double pz, long now) {
		int wantX0 = (int) Math.floor(px) - GRID / 2;
		int wantZ0 = (int) Math.floor(pz) - GRID / 2;
		if (gridX0 == Integer.MIN_VALUE) {
			gridX0 = wantX0;
			gridZ0 = wantZ0;
			if (!columns.read(gridX0, gridZ0, GRID, GRID, grid)) {
				gridX0 = Integer.MIN_VALUE;
			}
			stripeAt = now;
			return;
		}
		if (Math.abs(wantX0 - gridX0) >= RECENTER || Math.abs(wantZ0 - gridZ0) >= RECENTER) {
			shift(columns, wantX0, wantZ0);
		}
		if (now - stripeAt >= STRIPE_NANOS) {
			stripeAt = now;
			int rows = Math.min(STRIPE, GRID - nextStripe);
			if (columns.read(gridX0, gridZ0 + nextStripe, GRID, rows, strip)) {
				System.arraycopy(strip, 0, grid, nextStripe * GRID, rows * GRID);
			}
			nextStripe = (nextStripe + rows) % GRID;
		}
	}

	/** Move the grid to start at (x0, z0): what overlaps is kept, the rest read. */
	private void shift(Columns columns, int x0, int z0) {
		int dx = x0 - gridX0;
		int dz = z0 - gridZ0;
		if (Math.abs(dx) >= GRID || Math.abs(dz) >= GRID) {
			gridX0 = x0;
			gridZ0 = z0;
			columns.read(x0, z0, GRID, GRID, grid);
			return;
		}
		int[] old = grid.clone();
		java.util.Arrays.fill(grid, 0);
		for (int row = 0; row < GRID; row++) {
			int from = row + dz;
			if (from < 0 || from >= GRID) {
				continue;
			}
			int a = Math.max(0, -dx);
			int b = Math.min(GRID, GRID - dx);
			if (b > a) {
				System.arraycopy(old, from * GRID + a + dx, grid, row * GRID + a, b - a);
			}
		}
		gridX0 = x0;
		gridZ0 = z0;
		// The rows that came into view, then the columns.
		int[] buf = new int[GRID * GRID];
		if (dz != 0) {
			int rows = Math.abs(dz);
			int top = dz > 0 ? GRID - rows : 0;
			if (columns.read(x0, z0 + top, GRID, rows, buf)) {
				System.arraycopy(buf, 0, grid, top * GRID, rows * GRID);
			}
		}
		if (dx != 0) {
			int cols = Math.abs(dx);
			int left = dx > 0 ? GRID - cols : 0;
			if (columns.read(x0 + left, z0, cols, GRID, buf)) {
				for (int row = 0; row < GRID; row++) {
					System.arraycopy(buf, row * cols, grid, row * GRID + left, cols);
				}
			}
		}
	}

	/** The color of block (x, z), or 0 if it's outside the grid or not loaded. */
	int colorAt(int x, int z) {
		int gx = x - gridX0;
		int gz = z - gridZ0;
		if (gridX0 == Integer.MIN_VALUE || gx < 0 || gz < 0 || gx >= GRID || gz >= GRID) {
			return 0;
		}
		return grid[gz * GRID + gx];
	}

	/**
	 * Draw the picture: {@code blocks} across, centered on (px, pz), turned so that the way
	 * you face ({@code yaw}) is up when {@code rotate}, round or square, with {@code markers}.
	 */
	void compose(double px, double pz, double yaw, double blocks, boolean rotate, boolean round, List<Marker> markers,
			int arrowColor) {
		double perPixel = blocks / PICTURE;
		// Screen right and screen up, as directions on the ground (x, z).
		double[] right = {1, 0};
		double[] up = {0, -1};
		if (rotate) {
			double r = Math.toRadians(yaw);
			up = new double[] {-Math.sin(r), Math.cos(r)};
			right = new double[] {-up[1], up[0]};
		}
		double half = PICTURE / 2.0;
		for (int j = 0; j < PICTURE; j++) {
			double oy = (j + 0.5 - half) * perPixel;
			for (int i = 0; i < PICTURE; i++) {
				double ox = (i + 0.5 - half) * perPixel;
				double wx = px + ox * right[0] - oy * up[0];
				double wz = pz + ox * right[1] - oy * up[1];
				int c = colorAt((int) Math.floor(wx), (int) Math.floor(wz));
				int color = c == 0 ? UNLOADED : c;
				if (round) {
					double d = Math.hypot(i + 0.5 - half, j + 0.5 - half);
					// One pixel of soft edge.
					double cover = Math.max(0, Math.min(1, half - d));
					color = withAlpha(color, cover);
				}
				pixels[j * PICTURE + i] = color;
			}
		}
		double limit = half - 6;
		for (Marker m : markers) {
			double dx = (m.x - px) / perPixel;
			double dz = (m.z - pz) / perPixel;
			double sx = dx * right[0] + dz * right[1];
			double sy = -(dx * up[0] + dz * up[1]);
			// Further than the edge: pinned to it.
			double far = round ? Math.hypot(sx, sy) / limit : Math.max(Math.abs(sx), Math.abs(sy)) / limit;
			if (far > 1) {
				sx /= far;
				sy /= far;
			}
			double cx = half + sx;
			double cy = half + sy;
			if (Double.isNaN(m.yaw)) {
				disc(cx, cy, m.big ? 5 : 4, 0xFF000000);
				disc(cx, cy, m.big ? 4 : 3, m.color);
			} else {
				arrow(cx, cy, screenAngle(m.yaw, rotate, yaw), m.big ? 10 : 7, m.color);
			}
		}
		arrow(half, half, rotate ? 0 : screenAngle(yaw, false, 0), 11, arrowColor);
	}

	/** Which way a yaw points on the picture (radians, 0 = up, clockwise). */
	static double screenAngle(double markerYaw, boolean rotate, double viewYaw) {
		// Yaw 180 faces north, which is up on a north-up map.
		double deg = markerYaw - 180 - (rotate ? viewYaw - 180 : 0);
		return Math.toRadians(deg);
	}

	/** A dart pointing at {@code angle}: dark outline, then the fill. */
	private void arrow(double cx, double cy, double angle, double size, int color) {
		double s = Math.sin(angle);
		double c = Math.cos(angle);
		double[][] tip = {{0, -size}, {size * 0.62, size * 0.75}, {0, size * 0.38}, {-size * 0.62, size * 0.75}};
		double[] xs = new double[4];
		double[] ys = new double[4];
		for (int k = 0; k < 4; k++) {
			xs[k] = cx + tip[k][0] * c - tip[k][1] * s;
			ys[k] = cy + tip[k][0] * s + tip[k][1] * c;
		}
		polygon(xs, ys, 0xFF000000, 1.4);
		polygon(xs, ys, color, 0);
	}

	/** Fill a polygon (grown by {@code grow} pixels for an outline), with smooth edges. */
	private void polygon(double[] xs, double[] ys, int color, double grow) {
		int x0 = (int) Math.floor(min(xs) - grow - 1);
		int x1 = (int) Math.ceil(max(xs) + grow + 1);
		int y0 = (int) Math.floor(min(ys) - grow - 1);
		int y1 = (int) Math.ceil(max(ys) + grow + 1);
		for (int y = Math.max(0, y0); y < Math.min(PICTURE, y1); y++) {
			for (int x = Math.max(0, x0); x < Math.min(PICTURE, x1); x++) {
				// Four samples per pixel for the edge.
				int inside = 0;
				for (int k = 0; k < 4; k++) {
					double sx = x + 0.25 + (k & 1) * 0.5;
					double sy = y + 0.25 + (k >> 1) * 0.5;
					if (distanceInside(xs, ys, sx, sy) >= -grow) {
						inside++;
					}
				}
				if (inside > 0) {
					blend(x, y, color, inside / 4.0);
				}
			}
		}
	}

	/** How far (pixels) a point is inside a convex-or-dart polygon: negative outside. */
	private static double distanceInside(double[] xs, double[] ys, double px, double py) {
		boolean in = false;
		double nearest = Double.MAX_VALUE;
		int n = xs.length;
		for (int i = 0, j = n - 1; i < n; j = i++) {
			if ((ys[i] > py) != (ys[j] > py) && px < (xs[j] - xs[i]) * (py - ys[i]) / (ys[j] - ys[i]) + xs[i]) {
				in = !in;
			}
			nearest = Math.min(nearest, segment(px, py, xs[j], ys[j], xs[i], ys[i]));
		}
		return in ? nearest : -nearest;
	}

	private static double segment(double px, double py, double ax, double ay, double bx, double by) {
		double vx = bx - ax;
		double vy = by - ay;
		double len = vx * vx + vy * vy;
		double t = len == 0 ? 0 : Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / len));
		return Math.hypot(px - (ax + t * vx), py - (ay + t * vy));
	}

	private void disc(double cx, double cy, double r, int color) {
		for (int y = (int) Math.floor(cy - r - 1); y <= (int) Math.ceil(cy + r + 1); y++) {
			for (int x = (int) Math.floor(cx - r - 1); x <= (int) Math.ceil(cx + r + 1); x++) {
				double d = Math.hypot(x + 0.5 - cx, y + 0.5 - cy);
				double cover = Math.max(0, Math.min(1, r - d + 0.5));
				if (cover > 0) {
					blend(x, y, color, cover);
				}
			}
		}
	}

	private void blend(int x, int y, int color, double amount) {
		if (x < 0 || y < 0 || x >= PICTURE || y >= PICTURE) {
			return;
		}
		int i = y * PICTURE + x;
		int under = pixels[i];
		double a = amount * ((color >>> 24) / 255.0);
		int r = mix((under >> 16) & 255, (color >> 16) & 255, a);
		int g = mix((under >> 8) & 255, (color >> 8) & 255, a);
		int b = mix(under & 255, color & 255, a);
		int alpha = Math.max(under >>> 24, (int) Math.round(a * 255));
		pixels[i] = alpha << 24 | r << 16 | g << 8 | b;
	}

	private static int mix(int from, int to, double t) {
		return (int) Math.round(from + (to - from) * t);
	}

	private static int withAlpha(int color, double alpha) {
		return (int) Math.round(alpha * 255) << 24 | (color & 0xFFFFFF);
	}

	private static double min(double[] v) {
		double m = v[0];
		for (double x : v) {
			m = Math.min(m, x);
		}
		return m;
	}

	private static double max(double[] v) {
		double m = v[0];
		for (double x : v) {
			m = Math.max(m, x);
		}
		return m;
	}
}
