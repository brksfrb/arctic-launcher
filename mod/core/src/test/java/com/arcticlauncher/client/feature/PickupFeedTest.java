package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class PickupFeedTest {
	@Test
	void shortEnchantLinesAreKept() {
		assertNull(PickupFeed.shorten(null));
		assertEquals("Sharpness V, Unbreaking III", PickupFeed.shorten("Sharpness V, Unbreaking III"));
	}

	@Test
	void longEnchantLinesAreCutWithAnEllipsis() {
		String cut = PickupFeed.shorten("Sharpness V, Unbreaking III, Mending, Looting III, Fire Aspect II, Sweeping Edge III");
		assertEquals(PickupFeed.MAX_EXTRA, cut.length());
		assertTrue(cut.endsWith("…"));
	}
}
