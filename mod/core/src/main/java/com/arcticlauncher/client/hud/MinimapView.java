package com.arcticlauncher.client.hud;

import java.util.ArrayList;
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
	/** A pixel's four edge samples are at most this far from its middle (0.25 each way), with a little to spare. */
	private static final double SAMPLE_REACH = 0.36;
	/** The round map's shape: each pixel's alpha (in the top byte), one pixel of soft edge. Worked out once. */
	private static final int[] ROUND_ALPHA = roundAlpha();

	private static int[] roundAlpha() {
		int[] alpha = new int[PICTURE * PICTURE];
		double half = PICTURE / 2.0;
		for (int j = 0; j < PICTURE; j++) {
			for (int i = 0; i < PICTURE; i++) {
				double dx = i + 0.5 - half;
				double dy = j + 0.5 - half;
				double cover = Math.max(0, Math.min(1, half - Math.sqrt(dx * dx + dy * dy)));
				alpha[j * PICTURE + i] = (int) Math.round(cover * 255) << 24;
			}
		}
		return alpha;
	}

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

		/** Drawn exactly the same (the same bits, so NaN yaws match). */
		boolean sameAs(Marker o) {
			return Double.doubleToLongBits(x) == Double.doubleToLongBits(o.x) && Double.doubleToLongBits(z) == Double.doubleToLongBits(o.z)
					&& Double.doubleToLongBits(yaw) == Double.doubleToLongBits(o.yaw) && color == o.color && big == o.big;
		}
	}

	private final int[] grid = new int[GRID * GRID];
	private int gridX0 = Integer.MIN_VALUE;
	private int gridZ0;
	private int nextStripe;
	private long stripeAt;
	final int[] pixels = new int[PICTURE * PICTURE];
	private final int[] strip = new int[GRID * STRIPE];
	/** Bumped whenever {@link #grid} (or where it lies) changes, so the land picture knows when to be drawn again. */
	private int gridVersion;
	/**
	 * The land part of the picture (no markers), kept with what it was drawn from: standing still (or only
	 * others moving) the land is copied instead of sampled again for every pixel, every frame.
	 */
	private final int[] land = new int[PICTURE * PICTURE];
	private boolean landValid;
	private double landX;
	private double landZ;
	private double landYaw;
	private double landBlocks;
	private boolean landRotate;
	private boolean landRound;
	private int landVersion;
	/** Each pixel's offset from the middle, in pixels (the same for rows and columns). */
	private static final double[] OFFSET = offsets();
	/** Per column (x) and per row (y): offset × direction, worked out once per drawing. */
	private final double[] colX = new double[PICTURE];
	private final double[] colZ = new double[PICTURE];

	private static double[] offsets() {
		double[] o = new double[PICTURE];
		double half = PICTURE / 2.0;
		for (int i = 0; i < PICTURE; i++) {
			o[i] = i + 0.5 - half;
		}
		return o;
	}

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
			gridVersion++;
			stripeAt = now;
			return;
		}
		if (Math.abs(wantX0 - gridX0) >= RECENTER || Math.abs(wantZ0 - gridZ0) >= RECENTER) {
			shift(columns, wantX0, wantZ0);
		}
		if (now - stripeAt >= STRIPE_NANOS) {
			stripeAt = now;
			int rows = Math.min(STRIPE, GRID - nextStripe);
			if (columns.read(gridX0, gridZ0 + nextStripe, GRID, rows, strip) && !sameRows(strip, nextStripe * GRID, rows * GRID)) {
				System.arraycopy(strip, 0, grid, nextStripe * GRID, rows * GRID);
				gridVersion++;
			}
			nextStripe = (nextStripe + rows) % GRID;
		}
	}

	/** Does the grid already hold these colors from {@code at}? */
	private boolean sameRows(int[] fresh, int at, int count) {
		for (int k = 0; k < count; k++) {
			if (grid[at + k] != fresh[k]) {
				return false;
			}
		}
		return true;
	}

	/** Move the grid to start at (x0, z0): what overlaps is kept, the rest read. */
	private void shift(Columns columns, int x0, int z0) {
		gridVersion++;
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
		// The land only depends on where you are, the zoom, the shape, the grid, and (turning) your yaw.
		boolean sameLand = landValid && px == landX && pz == landZ && blocks == landBlocks && rotate == landRotate
				&& round == landRound && gridVersion == landVersion && (!rotate || yaw == landYaw);
		// Nothing at all changed (you and everyone on the map standing still): the picture is already right.
		if (sameLand && yaw == pictureYaw && arrowColor == pictureArrow && sameMarkers(markers)) {
			return;
		}
		pictureYaw = yaw;
		pictureArrow = arrowColor;
		pictureMarkers = new ArrayList<Marker>(markers);
		if (!sameLand) {
			land(px, pz, perPixel, right, up, round);
			landValid = true;
			landX = px;
			landZ = pz;
			landYaw = yaw;
			landBlocks = blocks;
			landRotate = rotate;
			landRound = round;
			landVersion = gridVersion;
		}
		System.arraycopy(land, 0, pixels, 0, land.length);
		double limit = half - 6;
		for (Marker m : markers) {
			double dx = (m.x - px) / perPixel;
			double dz = (m.z - pz) / perPixel;
			double sx = dx * right[0] + dz * right[1];
			double sy = -(dx * up[0] + dz * up[1]);
			// Further than the edge: pinned to it.
			double far = round ? Math.sqrt(sx * sx + sy * sy) / limit : Math.max(Math.abs(sx), Math.abs(sy)) / limit;
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

	/**
	 * Sample the grid for every pixel of the land picture. The same arithmetic as one formula per pixel
	 * ({@code px + ox * right.x - oy * up.x}, floored), with the per-column and per-row products worked out
	 * once and the floor done without Math.floor (a slow call on Java 8): the same pixels, bit for bit.
	 */
	private void land(double px, double pz, double perPixel, double[] right, double[] up, boolean round) {
		double r0 = right[0];
		double r1 = right[1];
		for (int i = 0; i < PICTURE; i++) {
			double ox = OFFSET[i] * perPixel;
			colX[i] = ox * r0;
			colZ[i] = ox * r1;
		}
		boolean empty = gridX0 == Integer.MIN_VALUE;
		int x0 = gridX0;
		int z0 = gridZ0;
		for (int j = 0; j < PICTURE; j++) {
			double oy = OFFSET[j] * perPixel;
			double rowX = oy * up[0];
			double rowZ = oy * up[1];
			int row = j * PICTURE;
			for (int i = 0; i < PICTURE; i++) {
				double wx = px + colX[i] - rowX;
				double wz = pz + colZ[i] - rowZ;
				int c = 0;
				if (!empty) {
					int gx = floor(wx) - x0;
					int gz = floor(wz) - z0;
					if (gx >= 0 && gz >= 0 && gx < GRID && gz < GRID) {
						c = grid[gz * GRID + gx];
					}
				}
				int color = c == 0 ? UNLOADED : c;
				if (round) {
					color = ROUND_ALPHA[row + i] | (color & 0xFFFFFF);
				}
				land[row + i] = color;
			}
		}
	}

	/** (int) Math.floor(v), for the block coordinates a map can show. */
	static int floor(double v) {
		int t = (int) v;
		return v < t && t != Integer.MIN_VALUE ? t - 1 : t;
	}

	/** What the picture in {@link #pixels} was last drawn with, beyond the land (null: not drawn). */
	private List<Marker> pictureMarkers;
	private double pictureYaw = Double.NaN;
	private int pictureArrow;

	private boolean sameMarkers(List<Marker> markers) {
		List<Marker> last = pictureMarkers;
		if (last == null || last.size() != markers.size()) {
			return false;
		}
		for (int i = 0; i < last.size(); i++) {
			if (!last.get(i).sameAs(markers.get(i))) {
				return false;
			}
		}
		return true;
	}

	/** Which way a yaw points on the picture (radians, 0 = up, clockwise). */
	static double screenAngle(double markerYaw, boolean rotate, double viewYaw) {
		// Yaw 180 faces north, which is up on a north-up map.
		double deg = markerYaw - 180 - (rotate ? viewYaw - 180 : 0);
		return Math.toRadians(deg);
	}

	/** The dark outline around an arrow, in pixels. */
	private static final double OUTLINE = 1.4;

	/** A dart pointing at {@code angle}: dark outline, then the fill. */
	private void arrow(double cx, double cy, double angle, double size, int color) {
		double s = Math.sin(angle);
		double c = Math.cos(angle);
		double[] xs = arrowXs;
		double[] ys = arrowYs;
		for (int k = 0; k < 4; k++) {
			double tx = TIP[k][0] * size;
			double ty = TIP[k][1] * size;
			xs[k] = cx + tx * c - ty * s;
			ys[k] = cy + tx * s + ty * c;
		}
		outlined(xs, ys, 0xFF000000, color);
	}

	/** The dart's corners for size 1: tip, right barb, notch, left barb. */
	private static final double[][] TIP = {{0, -1}, {0.62, 0.75}, {0, 0.38}, {-0.62, 0.75}};
	private final double[] arrowXs = new double[4];
	private final double[] arrowYs = new double[4];

	/**
	 * Fill a polygon in {@code outline}, grown by {@link #OUTLINE} pixels, and then itself in {@code color}
	 * on top, both with smooth edges. Each pixel is looked at once for the two.
	 */
	private void outlined(double[] xs, double[] ys, int outline, int color) {
		int x0 = (int) Math.floor(min(xs) - OUTLINE - 1);
		int x1 = (int) Math.ceil(max(xs) + OUTLINE + 1);
		int y0 = (int) Math.floor(min(ys) - OUTLINE - 1);
		int y1 = (int) Math.ceil(max(ys) + OUTLINE + 1);
		for (int y = Math.max(0, y0); y < Math.min(PICTURE, y1); y++) {
			int from = Math.max(0, x0);
			int to = Math.min(PICTURE, x1);
			if (cullRows) {
				// Only where the dart comes within reach of this row: a pixel further than OUTLINE + SAMPLE_REACH
				// from it is skipped below anyway (its middle is outside by more than that), so it isn't looked at.
				double[] span = rowSpan(xs, ys, y + 0.5 - REACH, y + 0.5 + REACH);
				if (span == null) {
					continue;
				}
				from = Math.max(from, (int) Math.floor(span[0] - REACH - 0.5));
				to = Math.min(to, (int) Math.ceil(span[1] + REACH - 0.5) + 1);
			}
			for (int x = from; x < to; x++) {
				// Four samples per pixel for the edges. The middle tells for most pixels: its samples are
				// within SAMPLE_REACH of it, so further in or out than that they all agree.
				double middle = distanceInside(xs, ys, x + 0.5, y + 0.5);
				if (middle + OUTLINE < -SAMPLE_REACH) {
					continue;
				}
				int inOutline = 4;
				int inFill = middle >= SAMPLE_REACH ? 4 : 0;
				if (middle + OUTLINE < SAMPLE_REACH || Math.abs(middle) < SAMPLE_REACH) {
					inOutline = 0;
					inFill = 0;
					for (int k = 0; k < 4; k++) {
						double d = distanceInside(xs, ys, x + 0.25 + (k & 1) * 0.5, y + 0.25 + (k >> 1) * 0.5);
						if (d >= -OUTLINE) {
							inOutline++;
						}
						if (d >= 0) {
							inFill++;
						}
					}
				}
				if (inOutline > 0) {
					blend(x, y, outline, inOutline / 4.0);
				}
				if (inFill > 0) {
					blend(x, y, color, inFill / 4.0);
				}
			}
		}
	}

	/** Off only in tests, to compare with every pixel of the box looked at. */
	static boolean cullRows = true;
	/** How far from the dart a pixel's middle can be and still be touched, with a margin for rounding. */
	private static final double REACH = OUTLINE + SAMPLE_REACH + 0.01;
	private final double[] span = new double[2];

	/** The x range of the polygon's part between heights lo and hi, or null when it has none there. */
	private double[] rowSpan(double[] xs, double[] ys, double lo, double hi) {
		double min = Double.MAX_VALUE;
		double max = -Double.MAX_VALUE;
		int n = xs.length;
		for (int i = 0, j = n - 1; i < n; j = i++) {
			double ax = xs[j];
			double ay = ys[j];
			double bx = xs[i];
			double by = ys[i];
			double t0;
			double t1;
			if (ay == by) {
				if (ay < lo || ay > hi) {
					continue;
				}
				t0 = 0;
				t1 = 1;
			} else {
				double ta = (lo - ay) / (by - ay);
				double tb = (hi - ay) / (by - ay);
				t0 = Math.max(0, Math.min(ta, tb));
				t1 = Math.min(1, Math.max(ta, tb));
				if (t0 > t1) {
					continue;
				}
			}
			double xa = ax + (bx - ax) * t0;
			double xb = ax + (bx - ax) * t1;
			min = Math.min(min, Math.min(xa, xb));
			max = Math.max(max, Math.max(xa, xb));
		}
		if (min > max) {
			return null;
		}
		span[0] = min;
		span[1] = max;
		return span;
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
			nearest = Math.min(nearest, segmentSquared(px, py, xs[j], ys[j], xs[i], ys[i]));
		}
		// One square root for the nearest edge. (Not Math.hypot per edge: on Java 8 that is a slow library
		// call, and this runs for every pixel of every arrow, every frame.)
		nearest = Math.sqrt(nearest);
		return in ? nearest : -nearest;
	}

	/** The squared distance from a point to a line segment. */
	private static double segmentSquared(double px, double py, double ax, double ay, double bx, double by) {
		double vx = bx - ax;
		double vy = by - ay;
		double len = vx * vx + vy * vy;
		double t = len == 0 ? 0 : Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / len));
		double dx = px - (ax + t * vx);
		double dy = py - (ay + t * vy);
		return dx * dx + dy * dy;
	}

	private void disc(double cx, double cy, double r, int color) {
		for (int y = (int) Math.floor(cy - r - 1); y <= (int) Math.ceil(cy + r + 1); y++) {
			for (int x = (int) Math.floor(cx - r - 1); x <= (int) Math.ceil(cx + r + 1); x++) {
				double dx = x + 0.5 - cx;
				double dy = y + 0.5 - cy;
				double cover = Math.max(0, Math.min(1, r - Math.sqrt(dx * dx + dy * dy) + 0.5));
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
