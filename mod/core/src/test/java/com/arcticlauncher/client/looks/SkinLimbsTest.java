package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

class SkinLimbsTest {
	private static final float EPS = 1e-3f;

	/** Collects vertex positions (in pixels, relative to the part) and their texture rows. */
	private static final class Box extends Xform {
		float minY = Float.MAX_VALUE;
		float maxY = -Float.MAX_VALUE;
		float minZ = Float.MAX_VALUE;
		float maxZ = -Float.MAX_VALUE;
		float minV = Float.MAX_VALUE;
		float maxV = -Float.MAX_VALUE;
		int vertices;

		@Override
		public void position(float px, float py, float pz) {
			x = px * 16;
			y = py * 16;
			z = pz * 16;
		}

		@Override
		public void normal(float nx, float ny, float nz) {
			x = nx;
			y = ny;
			z = nz;
		}

		@Override
		public void vertex(float vx, float vy, float vz, int argb, float u, float v, int light, float nx, float ny, float nz) {
			vertices++;
			minY = Math.min(minY, vy);
			maxY = Math.max(maxY, vy);
			minZ = Math.min(minZ, vz);
			maxZ = Math.max(maxZ, vz);
			minV = Math.min(minV, v * 64);
			maxV = Math.max(maxV, v * 64);
		}
	}

	private static Animation bend(String bone, float degrees) {
		return Animation.parse(new JsonParser().parse("{\"animations\":{\"a\":{\"animation_length\":1,\"bones\":{\"" + bone
				+ "\":{\"rotation\":[" + degrees + ",0,0]}}}}}"));
	}

	@Test
	void everyLimbBuilds() {
		for (SkinLimbs.Limb limb : SkinLimbs.Limb.values()) {
			for (boolean slim : new boolean[] {false, true}) {
				Box box = new Box();
				SkinLimbs.piece(limb, slim, true).emit(box, 0);
				// Two halves, base and outer layer, six faces of four corners.
				assertEquals(2 * 2 * 6 * 4, box.vertices, limb + " slim " + slim);
			}
		}
	}

	@Test
	void theBodySplitsAtItsMiddleWithTheJacketOutside() {
		Box box = new Box();
		SkinLimbs.piece(SkinLimbs.Limb.BODY, false, true).emit(box, 0);
		// The body: from its pivot (the neck) 12 down, its skin rows 16 to 32 and the jacket's 32 to 48.
		assertEquals(-0.25f, box.minY, EPS);
		assertEquals(12.25f, box.maxY, EPS);
		assertEquals(16f, box.minV, EPS);
		assertEquals(48f, box.maxV, EPS);
	}

	@Test
	void aStraightArmFillsTheGamesArmBox() {
		Box box = new Box();
		SkinLimbs.piece(SkinLimbs.Limb.RIGHT_ARM, false, false).emit(box, 0);
		// The game's right arm: 2 above its pivot to 10 below; its skin from row 16 to 32.
		assertEquals(-2f, box.minY, EPS);
		assertEquals(10f, box.maxY, EPS);
		assertEquals(16f, box.minV, EPS);
		assertEquals(32f, box.maxV, EPS);
	}

	@Test
	void theForearmBendsAtTheElbow() {
		Box box = new Box();
		SkinLimbs.piece(SkinLimbs.Limb.RIGHT_ARM, false, false).emit(box, 0, bend("rightForearm", -90), 0);
		// Bent forward 90° at the elbow (4 below the pivot): the hand ends 6 in front, nothing below 6.
		assertEquals(6f, box.maxY, EPS);
		assertEquals(-6f, box.minZ, 0.01f);
		assertTrue(box.maxZ <= 2.01f);
	}
}
