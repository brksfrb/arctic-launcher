package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class ZoomCurveTest {
	@Test
	void everyCurveStartsAtZeroAndEndsAtOne() {
		for (int kind = 0; kind < 5; kind++) {
			assertEquals(0f, Features.curve(kind, 0f), 1e-6f, "kind " + kind);
			assertEquals(1f, Features.curve(kind, 1f), 1e-3f, "kind " + kind);
		}
	}

	@Test
	void easeOutIsFastAtFirstAndEaseInOutIsSlowAtBothEnds() {
		assertTrue(Features.curve(2, 0.25f) > 0.25f);
		assertTrue(Features.curve(3, 0.25f) < 0.25f);
		assertTrue(Features.curve(3, 0.75f) > 0.75f);
		assertEquals(0.5f, Features.curve(3, 0.5f), 1e-6f);
	}

	@Test
	void instantJumpsInTheMiddle() {
		assertEquals(0f, Features.curve(0, 0.4f));
		assertEquals(1f, Features.curve(0, 0.6f));
	}
}
