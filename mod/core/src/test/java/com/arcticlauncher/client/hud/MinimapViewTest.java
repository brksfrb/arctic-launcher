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
}
