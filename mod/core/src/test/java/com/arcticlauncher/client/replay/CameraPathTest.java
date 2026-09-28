package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;
import org.junit.jupiter.api.Test;

class CameraPathTest {
	private static Keyframe key(int time, double x, float yaw) {
		return new Keyframe(time, new double[] {x, 64, 0, yaw, 0, 70});
	}

	@Test
	void passesThroughEveryKeyframe() {
		CameraPath path = new CameraPath(Arrays.asList(key(0, 0, 0), key(1000, 10, 0), key(2000, 0, 0)));
		assertEquals(0, path.at(0)[0], 1e-9);
		assertEquals(10, path.at(1000)[0], 1e-9);
		assertEquals(0, path.at(2000)[0], 1e-9);
		assertEquals(0, path.start());
		assertEquals(2000, path.end());
	}

	@Test
	void holdsTheEndsOutsideThePath() {
		CameraPath path = new CameraPath(Arrays.asList(key(500, 3, 0), key(900, 5, 0)));
		assertEquals(3, path.at(0)[0], 1e-9);
		assertEquals(5, path.at(5000)[0], 1e-9);
	}

	@Test
	void keyframesAreSortedByTime() {
		CameraPath path = new CameraPath(Arrays.asList(key(1000, 10, 0), key(0, 0, 0)));
		assertEquals(0, path.at(0)[0], 1e-9);
		assertEquals(5, path.at(500)[0], 1e-9);
	}

	@Test
	void turnsTheShortWayRound() {
		CameraPath path = new CameraPath(Arrays.asList(key(0, 0, 350), key(1000, 0, 10)));
		double mid = path.at(500)[3];
		assertEquals(360, mid, 1e-6);
	}

	@Test
	void doesNotChangeTheKeyframesItWasGiven() {
		Keyframe a = key(0, 0, 350);
		Keyframe b = key(1000, 0, 10);
		new CameraPath(Arrays.asList(a, b));
		assertEquals(10, b.yaw, 1e-9);
	}

	@Test
	void needsTwoKeyframes() {
		assertFalse(new CameraPath(Arrays.asList(key(0, 0, 0))).usable());
		assertTrue(new CameraPath(Arrays.asList(key(0, 0, 0), key(1, 0, 0))).usable());
	}
}
