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
}
