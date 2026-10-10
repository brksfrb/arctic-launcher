package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

class RigTest {
	private static final float EPS = 1e-3f;

	private static Animation anim(String bones) {
		return Animation.parse(new JsonParser().parse(
				"{\"animations\":{\"a\":{\"animation_length\":1,\"bones\":{" + bones + "}}}}"));
	}

	/** The game's rest pose: head, body, arms, legs pivots in model space. */
	private static float[][] restPivots() {
		return new float[][] {{0, 0, 0}, {0, 0, 0}, {-5, 2, 0}, {5, 2, 0}, {-1.9f, 12, 0}, {1.9f, 12, 0}};
	}

	@Test
	void eulerAnglesSurviveARoundTrip() {
		float[] a = Rig.angles(Rig.euler(0.3f, -0.7f, 1.1f));
		assertEquals(0.3f, a[0], EPS);
		assertEquals(-0.7f, a[1], EPS);
		assertEquals(1.1f, a[2], EPS);
	}

	@Test
	void onlyTorsoOrRootEmotesUseTheRig() {
		assertTrue(Rig.expressive(anim("\"torso\":{\"rotation\":[10,0,0]}")));
		assertTrue(Rig.expressive(anim("\"root\":{\"position\":[0,1,0]}")));
		assertFalse(Rig.expressive(anim("\"body\":{\"rotation\":[10,0,0]}")));
	}

	@Test
	void aBowCarriesTheHeadForwardAndDownButNotTheLegs() {
		float[][] pivots = restPivots();
		float[][] rotations = new float[6][3];
		Rig.pose(anim("\"torso\":{\"rotation\":[90,0,0]}"), 0, pivots, rotations);
		// Bent 90° at the waist (y 12): the neck ends up 12 in front of the waist, at waist height.
		assertEquals(12f, pivots[Rig.HEAD][1], EPS);
		assertEquals(-12f, pivots[Rig.HEAD][2], EPS);
		assertEquals((float) Math.PI / 2, rotations[Rig.HEAD][0], EPS);
		assertEquals(12f, pivots[Rig.RIGHT_LEG][1], EPS);
		assertEquals(0f, rotations[Rig.RIGHT_LEG][0], EPS);
	}

	@Test
	void theRootLiftsAndTurnsTheWholePlayer() {
		float[][] pivots = restPivots();
		float[][] rotations = new float[6][3];
		Rig.pose(anim("\"root\":{\"rotation\":[0,180,0],\"position\":[0,4,0]}"), 0, pivots, rotations);
		// Up 4 is -4 in the game's y-down model; a half turn mirrors x.
		assertEquals(-2f, pivots[Rig.RIGHT_ARM][1], EPS);
		assertEquals(5f, pivots[Rig.RIGHT_ARM][0], EPS);
		assertEquals((float) Math.PI, Math.abs(rotations[Rig.HEAD][1]) + Math.abs(rotations[Rig.HEAD][0]), 0.01f);
	}

	@Test
	void aSplitBodysPelvisCarriesTheLegsAndItsChestTheHead() {
		float[][] pivots = restPivots();
		float[][] rotations = new float[6][3];
		Rig.pose(anim("\"chest\":{\"rotation\":[0,0,0]},\"pelvis\":{\"rotation\":[90,0,0]}"), 0, pivots, rotations);
		// Turned 90° about the body's middle (y 6): the hips (y 12) swing 6 behind it; the head stays.
		assertEquals(6f, pivots[Rig.RIGHT_LEG][1], EPS);
		assertEquals(6f, pivots[Rig.RIGHT_LEG][2], EPS);
		assertEquals((float) Math.PI / 2, rotations[Rig.RIGHT_LEG][0], EPS);
		assertEquals(0f, pivots[Rig.HEAD][1], EPS);
		assertEquals(0f, rotations[Rig.HEAD][0], EPS);
	}
}
