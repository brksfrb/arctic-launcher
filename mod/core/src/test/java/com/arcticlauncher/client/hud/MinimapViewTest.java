package com.arcticlauncher.client.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Collections;
import org.junit.jupiter.api.Test;

class MinimapViewTest {
	/** A world whose color says where it is: x in the low byte, z in the next. */
	private static int colorOf(int x, int z) {
		return 0xFF000000 | (z & 0xFF) << 8 | (x & 0xFF);
	}

	private static final MinimapView.Columns WORLD = (x0, z0, w, h, out) -> {
		for (int z = 0; z < h; z++) {
			for (int x = 0; x < w; x++) {
				out[z * w + x] = colorOf(x0 + x, z0 + z);
			}
		}
		return true;
	};

	@Test
	void theGridFollowsYouKeepingWhatItHad() {
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		assertEquals(colorOf(10, -20), v.colorAt(10, -20));
		// Far enough to move the grid: what was kept and what was read both match the world.
		v.update(WORLD, 40.5, -30.5, 1);
		for (int[] p : new int[][] {{40, -30}, {40 + 90, -30}, {40 - 90, -30 - 90}, {0, 0}}) {
			assertEquals(colorOf(p[0], p[1]), v.colorAt(p[0], p[1]), p[0] + "," + p[1]);
		}
	}

	private static int pixel(MinimapView v, int x, int y) {
		return v.pixels[y * MinimapView.PICTURE + x];
	}

	@Test
	void northIsUpUnlessItTurnsWithYou() {
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		int mid = MinimapView.PICTURE / 2;
		// 64 blocks across: the top-middle pixel is about 30 blocks north (z = -30).
		v.compose(0.5, 0.5, 0, 64, false, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		int north = pixel(v, mid, 10);
		assertEquals(0, north & 0xFF, "x stays 0 straight up");
		assertTrue(((north >> 8) & 0xFF) > 0x80, "z is negative (wrapped) up there");
		// Facing east (yaw 270), turning with you: east is up, so up there x is large, z is 0.
		v.compose(0.5, 0.5, 270, 64, true, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		int up = pixel(v, mid, 10);
		assertTrue((up & 0xFF) > 20 && (up & 0xFF) < 40, "x ahead: " + (up & 0xFF));
		assertEquals(0, (up >> 8) & 0xFF);
	}

	@Test
	void aRoundMapIsSeeThroughOutsideItsCircle() {
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		v.compose(0.5, 0.5, 0, 64, false, true, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertEquals(0, pixel(v, 0, 0) >>> 24, "corner");
		assertEquals(255, pixel(v, MinimapView.PICTURE / 2, 20) >>> 24, "inside");
	}

	@Test
	void arrowsPointTheWayTheyFace() {
		// North-up: north is up (0), east right (90 degrees), south down.
		assertEquals(0, Math.toDegrees(MinimapView.screenAngle(180, false, 0)), 1e-9);
		assertEquals(90, Math.toDegrees(MinimapView.screenAngle(270, false, 0)), 1e-9);
		// Turning with you: your own way is always up.
		assertEquals(0, Math.toDegrees(MinimapView.screenAngle(37, true, 37)), 1e-9);
	}

	@Test
	void theRoundEdgeIsOnePixelSoft() {
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		v.compose(0.5, 0.5, 0, 64, false, true, Collections.<MinimapView.Marker>emptyList(), 0);
		double half = MinimapView.PICTURE / 2.0;
		// Away from the middle (where your own arrow is): the land's own color, see-through by how far out it is.
		for (int y = 0; y < MinimapView.PICTURE; y++) {
			for (int x = 0; x < 40; x++) {
				double cover = Math.max(0, Math.min(1, half - Math.hypot(x + 0.5 - half, y + 0.5 - half)));
				assertEquals((int) Math.round(cover * 255), pixel(v, x, y) >>> 24, x + "," + y);
			}
		}
	}

	@Test
	void anArrowIsSolidInsideAndSmoothAtItsEdge() {
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		// Another player 20 blocks east, facing north, in red: 60 pixels right of the middle at 64 blocks across.
		java.util.List<MinimapView.Marker> one = Collections.singletonList(new MinimapView.Marker(20.5, 0.5, 180, 0xFFFF0000, false));
		v.compose(0.5, 0.5, 0, 64, false, false, one, 0);
		int mid = MinimapView.PICTURE / 2;
		assertEquals(0xFFFF0000, pixel(v, mid + 60, mid), "the arrow's middle");
		assertEquals(0xFF000000, pixel(v, mid + 60, mid - 8), "its outline, past the tip");
		int partly = 0;
		for (int y = mid - 12; y < mid + 12; y++) {
			for (int x = mid + 48; x < mid + 72; x++) {
				int red = (pixel(v, x, y) >> 16) & 0xFF;
				int blue = pixel(v, x, y) & 0xFF;
				if (red > 0 && red < 255 && blue == 0) {
					partly++;
				}
			}
		}
		assertTrue(partly > 4, "blended edge pixels: " + partly);
	}
	/** The land as it was drawn before the cache: one formula per pixel, with Math.floor. */
	private static int[] referenceLand(MinimapView v, double px, double pz, double yaw, double blocks, boolean rotate, boolean round) {
		int n = MinimapView.PICTURE;
		int[] out = new int[n * n];
		double perPixel = blocks / n;
		double[] right = {1, 0};
		double[] up = {0, -1};
		if (rotate) {
			double r = Math.toRadians(yaw);
			up = new double[] {-Math.sin(r), Math.cos(r)};
			right = new double[] {-up[1], up[0]};
		}
		double half = n / 2.0;
		for (int j = 0; j < n; j++) {
			double oy = (j + 0.5 - half) * perPixel;
			for (int i = 0; i < n; i++) {
				double ox = (i + 0.5 - half) * perPixel;
				double wx = px + ox * right[0] - oy * up[0];
				double wz = pz + ox * right[1] - oy * up[1];
				int c = v.colorAt((int) Math.floor(wx), (int) Math.floor(wz));
				int color = c == 0 ? MinimapView.UNLOADED : c;
				if (round) {
					double dx = i + 0.5 - half;
					double dy = j + 0.5 - half;
					double cover = Math.max(0, Math.min(1, half - Math.sqrt(dx * dx + dy * dy)));
					color = (int) Math.round(cover * 255) << 24 | (color & 0xFFFFFF);
				}
				out[j * n + i] = color;
			}
		}
		return out;
	}

	/** Every pixel but the middle, where your own arrow is drawn over the land. */
	private static void assertSameLand(int[] want, MinimapView v, String what) {
		int n = MinimapView.PICTURE;
		for (int y = 0; y < n; y++) {
			for (int x = 0; x < n; x++) {
				if (Math.abs(x - n / 2) < 16 && Math.abs(y - n / 2) < 16) {
					continue;
				}
				if (want[y * n + x] != v.pixels[y * n + x]) {
					throw new AssertionError(what + ": pixel " + x + "," + y + " " + Integer.toHexString(want[y * n + x]) + " != "
							+ Integer.toHexString(v.pixels[y * n + x]));
				}
			}
		}
	}

	@Test
	void theFasterLandIsTheSamePictureBitForBit() {
		java.util.Random rnd = new java.util.Random(42);
		MinimapView v = new MinimapView();
		for (int k = 0; k < 300; k++) {
			// Near the origin, far out, and on the negative side (where floor and a cast differ).
			double scale = k % 3 == 0 ? 50 : k % 3 == 1 ? 3_000_000 : 700;
			double px = (rnd.nextDouble() - 0.5) * 2 * scale;
			double pz = (rnd.nextDouble() - 0.5) * 2 * scale;
			if (k % 7 == 0) {
				// Exactly on a block edge.
				px = Math.rint(px);
				pz = Math.rint(pz);
			}
			double yaw = rnd.nextDouble() * 720 - 360;
			double blocks = new double[] {32, 48, 64, 96, 128, 160, 33.3}[k % 7];
			boolean rotate = rnd.nextBoolean();
			boolean round = rnd.nextBoolean();
			v.update(WORLD, px, pz, k * 1_000_000_000L);
			v.compose(px, pz, yaw, blocks, rotate, round, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
			assertSameLand(referenceLand(v, px, pz, yaw, blocks, rotate, round), v, "case " + k);
		}
	}

	@Test
	void theKeptLandIsDrawnAgainWhenAnythingItShowsChanges() {
		int[] world = new int[1];
		MinimapView.Columns changing = (x0, z0, w, h, out) -> {
			for (int z = 0; z < h; z++) {
				for (int x = 0; x < w; x++) {
					out[z * w + x] = colorOf(x0 + x, z0 + z) ^ world[0];
				}
			}
			return true;
		};
		MinimapView v = new MinimapView();
		v.update(changing, 0.5, 0.5, 0);
		v.compose(0.5, 0.5, 0, 64, false, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 0, 64, false, false), v, "first");
		// A marker only: the land under it stays, the marker is drawn on a fresh copy each time.
		java.util.List<MinimapView.Marker> one = java.util.Arrays.asList(new MinimapView.Marker(20.5, 20.5, Double.NaN, 0xFFFF0000, false));
		v.compose(0.5, 0.5, 0, 64, false, false, one, 0xFFFFFFFF);
		int at = pixelAtBlock(20, 20, 64);
		assertTrue(v.pixels[at] != referenceLand(v, 0.5, 0.5, 0, 64, false, false)[at], "marker drawn");
		v.compose(0.5, 0.5, 0, 64, false, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 0, 64, false, false), v, "marker gone");
		// The world changes: once the refresh stripes have passed over it, the picture shows it.
		world[0] = 0x00ABCDEF;
		for (int k = 1; k <= 8; k++) {
			v.update(changing, 0.5, 0.5, k * 200_000_000L);
		}
		v.compose(0.5, 0.5, 0, 64, false, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 0, 64, false, false), v, "world changed");
		assertEquals(colorOf(20, 20) ^ 0x00ABCDEF, v.colorAt(20, 20));
		// Turning matters only when the map turns with you; zoom and shape always.
		v.compose(0.5, 0.5, 90, 64, false, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 90, 64, false, false), v, "yaw, north up");
		v.compose(0.5, 0.5, 90, 64, true, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 90, 64, true, false), v, "yaw, turning");
		v.compose(0.5, 0.5, 91, 64, true, false, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 91, 64, true, false), v, "turned a degree");
		v.compose(0.5, 0.5, 91, 96, true, true, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 0.5, 0.5, 91, 96, true, true), v, "zoom and round");
		v.compose(1.25, 0.5, 91, 96, true, true, Collections.<MinimapView.Marker>emptyList(), 0xFFFFFFFF);
		assertSameLand(referenceLand(v, 1.25, 0.5, 91, 96, true, true), v, "moved a little");
	}

	/** The picture pixel that shows block (x, z), north up, centered on (0.5, 0.5). */
	private static int pixelAtBlock(int x, int z, double blocks) {
		double perPixel = blocks / MinimapView.PICTURE;
		int i = (int) ((x + 0.5 - 0.5) / perPixel + MinimapView.PICTURE / 2.0);
		int j = (int) ((z + 0.5 - 0.5) / perPixel + MinimapView.PICTURE / 2.0);
		return j * MinimapView.PICTURE + i;
	}

	@Test
	void theFloorIsMathFloor() {
		java.util.Random rnd = new java.util.Random(7);
		double[] edges = {0, -0.0, 0.5, -0.5, 1, -1, -1e-12, 1e-12, 29_999_999.5, -29_999_999.5, -2147483648.0, -2147483648.5,
				2147483647.0, 2147483647.9, Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY};
		for (double d : edges) {
			assertEquals((int) Math.floor(d), MinimapView.floor(d), "floor(" + d + ")");
		}
		for (int k = 0; k < 100_000; k++) {
			double d = (rnd.nextDouble() - 0.5) * 6e7;
			assertEquals((int) Math.floor(d), MinimapView.floor(d), "floor(" + d + ")");
		}
	}
	private static java.util.List<MinimapView.Marker> randomMarkers(java.util.Random rnd, double px, double pz, int n) {
		java.util.List<MinimapView.Marker> out = new java.util.ArrayList<MinimapView.Marker>();
		for (int k = 0; k < n; k++) {
			// Near, on the edge and far away (pinned to the edge); arrows and dots, both sizes.
			double r = k % 3 == 0 ? rnd.nextDouble() * 20 : k % 3 == 1 ? 30 + rnd.nextDouble() * 5 : 200 + rnd.nextDouble() * 500;
			double a = rnd.nextDouble() * Math.PI * 2;
			double yaw = k % 5 == 0 ? Double.NaN : rnd.nextDouble() * 720 - 360;
			out.add(new MinimapView.Marker(px + Math.cos(a) * r, pz + Math.sin(a) * r, yaw, 0xFF000000 | rnd.nextInt(0xFFFFFF), k % 2 == 0));
		}
		return out;
	}

	@Test
	void arrowsDrawnOnlyWhereTheyReachAreTheSameAsLookingAtEveryPixel() {
		java.util.Random rnd = new java.util.Random(3);
		try {
			for (int k = 0; k < 60; k++) {
				double px = rnd.nextDouble() * 100 - 50;
				double pz = rnd.nextDouble() * 100 - 50;
				double yaw = rnd.nextDouble() * 360;
				boolean rotate = k % 2 == 0;
				boolean round = k % 3 == 0;
				java.util.List<MinimapView.Marker> markers = randomMarkers(rnd, px, pz, 40);
				MinimapView.cullRows = false;
				MinimapView all = new MinimapView();
				all.update(WORLD, px, pz, 0);
				all.compose(px, pz, yaw, 64, rotate, round, markers, 0xFF22AAFF);
				MinimapView.cullRows = true;
				MinimapView culled = new MinimapView();
				culled.update(WORLD, px, pz, 0);
				culled.compose(px, pz, yaw, 64, rotate, round, markers, 0xFF22AAFF);
				org.junit.jupiter.api.Assertions.assertArrayEquals(all.pixels, culled.pixels, "case " + k);
			}
		} finally {
			MinimapView.cullRows = true;
		}
	}

	@Test
	void anUnchangedPictureIsKeptAndAnyChangeRedrawsIt() {
		java.util.Random rnd = new java.util.Random(5);
		java.util.List<MinimapView.Marker> markers = randomMarkers(rnd, 0.5, 0.5, 30);
		MinimapView v = new MinimapView();
		v.update(WORLD, 0.5, 0.5, 0);
		v.compose(0.5, 0.5, 30, 64, true, true, markers, 0xFFFFFFFF);
		int[] first = v.pixels.clone();
		// The same again (a new list with equal markers): the same picture.
		v.compose(0.5, 0.5, 30, 64, true, true, new java.util.ArrayList<MinimapView.Marker>(markers), 0xFFFFFFFF);
		org.junit.jupiter.api.Assertions.assertArrayEquals(first, v.pixels);
		// Each change on its own gives what a fresh view draws.
		java.util.List<MinimapView.Marker> moved = new java.util.ArrayList<MinimapView.Marker>(markers);
		MinimapView.Marker m = moved.get(0);
		moved.set(0, new MinimapView.Marker(m.x + 0.25, m.z, m.yaw, m.color, m.big));
		Object[][] cases = {{30.0, moved, 0xFFFFFFFF, true}, {31.0, markers, 0xFFFFFFFF, true}, {30.0, markers, 0xFF00FF00, true},
				{30.0, markers.subList(0, 29), 0xFFFFFFFF, true}, {31.0, markers, 0xFFFFFFFF, false}};
		for (Object[] c : cases) {
			@SuppressWarnings("unchecked")
			java.util.List<MinimapView.Marker> ms = (java.util.List<MinimapView.Marker>) c[1];
			v.compose(0.5, 0.5, (Double) c[0], 64, (Boolean) c[3], true, ms, (Integer) c[2]);
			MinimapView fresh = new MinimapView();
			fresh.update(WORLD, 0.5, 0.5, 0);
			fresh.compose(0.5, 0.5, (Double) c[0], 64, (Boolean) c[3], true, ms, (Integer) c[2]);
			org.junit.jupiter.api.Assertions.assertArrayEquals(fresh.pixels, v.pixels, java.util.Arrays.toString(c));
		}
	}
}
