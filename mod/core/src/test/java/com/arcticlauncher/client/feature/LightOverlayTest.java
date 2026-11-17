package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class LightOverlayTest {
	@Test
	void classifiesLikeTheGameSpawnRules() {
		// 1.18+: block light must be 0.
		assertEquals(LightOverlay.RED, LightOverlay.classify(0, 0, 0, 0));
		assertEquals(LightOverlay.YELLOW, LightOverlay.classify(0, 15, 0, 0));
		assertEquals(LightOverlay.RED, LightOverlay.classify(0, 15, 11, 0));
		assertEquals(LightOverlay.GREEN, LightOverlay.classify(1, 0, 0, 0));
		// Before 1.18: 7 or less.
		assertEquals(LightOverlay.RED, LightOverlay.classify(7, 0, 0, 7));
		assertEquals(LightOverlay.GREEN, LightOverlay.classify(8, 0, 0, 7));
		assertEquals(LightOverlay.YELLOW, LightOverlay.classify(3, 12, 0, 7));
		// Sky light at the limit still spawns.
		assertEquals(LightOverlay.RED, LightOverlay.classify(0, 7, 0, 0));
		assertEquals(LightOverlay.YELLOW, LightOverlay.classify(0, 8, 0, 0));
	}

	private static LightOverlay.Cells cells(int... xyzKindLight) {
		int n = xyzKindLight.length / 5;
		int[] xyz = new int[n * 3];
		byte[] kind = new byte[n];
		byte[] light = new byte[n];
		for (int i = 0; i < n; i++) {
			xyz[i * 3] = xyzKindLight[i * 5];
			xyz[i * 3 + 1] = xyzKindLight[i * 5 + 1];
			xyz[i * 3 + 2] = xyzKindLight[i * 5 + 2];
			kind[i] = (byte) xyzKindLight[i * 5 + 3];
			light[i] = (byte) xyzKindLight[i * 5 + 4];
		}
		return new LightOverlay.Cells(10, 64, -5, n, xyz, kind, light, 1);
	}

	@Test
	void squaresAreOneQuadFloatingJustAboveTheFace() {
		LightOverlay.Mesh m = new LightOverlay.Mesh();
		LightOverlay.build(cells(1, 0, 2, LightOverlay.RED, 0, -3, -1, 4, LightOverlay.GREEN, 9), LightOverlay.SQUARES, m);
		assertEquals(8, m.vertices);
		assertEquals(10, m.originX);
		for (int v = 0; v < 4; v++) {
			float y = m.pos[v * 3 + 1];
			assertTrue(y > 0f && y < 0.1f, "above the face: " + y);
			assertTrue(m.pos[v * 3] >= 1f && m.pos[v * 3] <= 2f);
		}
		assertEquals(LightOverlay.COLOR_RED, m.color[0]);
		assertEquals(LightOverlay.COLOR_GREEN, m.color[4]);
	}

	@Test
	void crossesAreTwoQuadsAndNumbersOnePerLitPixel() {
		LightOverlay.Mesh m = new LightOverlay.Mesh();
		LightOverlay.build(cells(0, 0, 0, LightOverlay.RED, 3), LightOverlay.CROSSES, m);
		assertEquals(8, m.vertices);
		LightOverlay.build(cells(0, 0, 0, LightOverlay.RED, 7), LightOverlay.NUMBERS, m);
		assertEquals(4 * 7, m.vertices, "a 7 has seven lit pixels");
		LightOverlay.build(cells(0, 0, 0, LightOverlay.RED, 12), LightOverlay.NUMBERS, m);
		assertEquals(4 * (8 + 11), m.vertices, "1 lights eight pixels, 2 lights eleven");
	}

	@Test
	void numbersStayInsideTheirBlock() {
		LightOverlay.Mesh m = new LightOverlay.Mesh();
		for (int v = 0; v <= 15; v++) {
			LightOverlay.build(cells(0, 0, 0, LightOverlay.GREEN, v), LightOverlay.NUMBERS, m);
			for (int i = 0; i < m.vertices; i++) {
				assertTrue(m.pos[i * 3] >= 0f && m.pos[i * 3] <= 1f, "x " + m.pos[i * 3] + " for " + v);
				assertTrue(m.pos[i * 3 + 2] >= 0f && m.pos[i * 3 + 2] <= 1f, "z " + m.pos[i * 3 + 2] + " for " + v);
			}
		}
	}

	/** Where each lit pixel of a number lands, as a set of rounded corner points. */
	private static java.util.Set<String> corners(LightOverlay.Mesh m) {
		java.util.Set<String> out = new java.util.TreeSet<String>();
		for (int i = 0; i < m.vertices; i++) {
			out.add(Math.round(m.pos[i * 3] * 100) + "," + Math.round(m.pos[i * 3 + 2] * 100));
		}
		return out;
	}

	@Test
	void numbersTurnToReadFromWhereYouLook() {
		LightOverlay.Mesh north = new LightOverlay.Mesh();
		LightOverlay.Mesh south = new LightOverlay.Mesh();
		LightOverlay.Mesh east = new LightOverlay.Mesh();
		LightOverlay.Mesh west = new LightOverlay.Mesh();
		LightOverlay.Cells c = cells(0, 0, 0, LightOverlay.RED, 7);
		LightOverlay.build(c, LightOverlay.NUMBERS, LightOverlay.NORTH, north);
		LightOverlay.build(c, LightOverlay.NUMBERS, LightOverlay.SOUTH, south);
		LightOverlay.build(c, LightOverlay.NUMBERS, LightOverlay.EAST, east);
		LightOverlay.build(c, LightOverlay.NUMBERS, LightOverlay.WEST, west);
		// The old (north only) layout is kept looking north.
		LightOverlay.Mesh old = new LightOverlay.Mesh();
		LightOverlay.build(c, LightOverlay.NUMBERS, old);
		assertEquals(corners(old), corners(north));
		// Looking south it's the north one turned half round (x and z mirrored about the block's middle).
		java.util.Set<String> turned = new java.util.TreeSet<String>();
		for (int i = 0; i < north.vertices; i++) {
			turned.add(Math.round((1 - north.pos[i * 3]) * 100) + "," + Math.round((1 - north.pos[i * 3 + 2]) * 100));
		}
		assertEquals(turned, corners(south));
		// A 7's top bar is at the far edge from you: north (-z) looking north, +x looking east, -x looking west.
		assertTrue(minZ(north) < 0.4f && maxZ(north) > 0.6f);
		assertEquals(north.vertices, east.vertices);
		assertTrue(farthest(east, 1, 0) > farthest(east, -1, 0), "the 7's bar lies east, away from you, looking east");
		assertTrue(farthest(west, -1, 0) > farthest(west, 1, 0), "the 7's bar lies west looking west");
		assertTrue(farthest(north, 0, -1) > farthest(north, 0, 1), "the 7's bar lies north looking north");
		for (LightOverlay.Mesh m : new LightOverlay.Mesh[] {north, south, east, west}) {
			for (int i = 0; i < m.vertices; i++) {
				assertTrue(m.pos[i * 3] >= 0f && m.pos[i * 3] <= 1f && m.pos[i * 3 + 2] >= 0f && m.pos[i * 3 + 2] <= 1f);
			}
		}
	}

	/** How many vertices sit at the far edge along (dx, dz): the 7's top bar makes that side heavier. */
	private static int farthest(LightOverlay.Mesh m, int dx, int dz) {
		float best = -9;
		for (int i = 0; i < m.vertices; i++) {
			best = Math.max(best, m.pos[i * 3] * dx + m.pos[i * 3 + 2] * dz);
		}
		int n = 0;
		for (int i = 0; i < m.vertices; i++) {
			if (Math.abs(m.pos[i * 3] * dx + m.pos[i * 3 + 2] * dz - best) < 1e-4f) {
				n++;
			}
		}
		return n;
	}

	private static float minZ(LightOverlay.Mesh m) {
		float v = 9;
		for (int i = 0; i < m.vertices; i++) {
			v = Math.min(v, m.pos[i * 3 + 2]);
		}
		return v;
	}

	private static float maxZ(LightOverlay.Mesh m) {
		float v = -9;
		for (int i = 0; i < m.vertices; i++) {
			v = Math.max(v, m.pos[i * 3 + 2]);
		}
		return v;
	}

	@Test
	void facingFollowsTheGameYaw() {
		assertEquals(LightOverlay.SOUTH, LightOverlay.facing(0));
		assertEquals(LightOverlay.WEST, LightOverlay.facing(90));
		assertEquals(LightOverlay.NORTH, LightOverlay.facing(180));
		assertEquals(LightOverlay.NORTH, LightOverlay.facing(-180));
		assertEquals(LightOverlay.EAST, LightOverlay.facing(-90));
		assertEquals(LightOverlay.EAST, LightOverlay.facing(270));
		assertEquals(LightOverlay.SOUTH, LightOverlay.facing(400));
		assertEquals(LightOverlay.WEST, LightOverlay.facing(44 + 90));
	}

	/** A flat world: a floor at y 63, dark (block light 0) except where {@link #lit} says. */
	private static final class World implements LightOverlay.Probe {
		int lit;
		int scans;

		@Override
		public boolean canSpawn(int x, int y, int z) {
			if (x == 0 && z == 0 && y == 64) {
				scans++;
			}
			return y == 64;
		}

		@Override
		public int blockLight(int x, int y, int z) {
			return lit;
		}

		@Override
		public int skyLight(int x, int y, int z) {
			return 0;
		}

		@Override
		public int skyDarken() {
			return 0;
		}

		@Override
		public int blockLimit() {
			return 0;
		}

		@Override
		public int minY() {
			return 0;
		}

		@Override
		public int maxY() {
			return 256;
		}
	}

	@Test
	void aBlockChangeNearbyRescansWithinAFewTicks() {
		final World world = new World();
		com.arcticlauncher.client.config.ClientConfig c = new com.arcticlauncher.client.config.ClientConfig();
		c.lightOverlay = true;
		c.lightRange = 4;
		c.lightKey = "key.keyboard.unknown";
		com.arcticlauncher.client.Platform p = (com.arcticlauncher.client.Platform) java.lang.reflect.Proxy.newProxyInstance(
				LightOverlayTest.class.getClassLoader(), new Class<?>[] {com.arcticlauncher.client.Platform.class}, (proxy, m, args) -> {
					switch (m.getName()) {
						case "lightOverlayWorks":
							return true;
						case "lightProbe":
							return world;
						case "position":
							return new double[] {0.5, 64, 0.5, 180, 0};
						case "camera":
							return null;
						case "isKeyDown":
							return false;
						default:
							Class<?> r = m.getReturnType();
							return r == boolean.class ? false : r == int.class ? 0 : r == float.class ? 0f : r == double.class ? 0d : r == long.class ? 0L : null;
					}
				});
		LightOverlay o = new LightOverlay();
		o.tick(p, c, false);
		assertTrue(o.mesh().vertices > 0);
		assertEquals(LightOverlay.COLOR_RED, o.mesh().color[0]);
		// A torch: everything lit; without the change it would wait for the 10 tick rescan.
		world.lit = 14;
		o.blockChanged(1, 64, 1);
		assertTrue(o.changePending());
		int before = world.scans;
		for (int t = 0; t < LightOverlay.CHANGE_DELAY; t++) {
			o.tick(p, c, false);
		}
		assertTrue(world.scans > before, "rescanned " + LightOverlay.CHANGE_DELAY + " ticks after the change");
		assertEquals(LightOverlay.COLOR_GREEN, o.mesh().color[0]);
		// Far away changes are ignored.
		o.blockChanged(500, 64, 500);
		assertTrue(!o.changePending());
	}
}
