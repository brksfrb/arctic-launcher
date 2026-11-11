package com.arcticlauncher.legacy;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class LegacyWindowSizeTest {
	private final LegacyWindowSize size = new LegacyWindowSize();
	private final LegacyWindowSize.Resize noRepair = (w, h) -> fail("unexpected resize repair");

	@Test
	void ignoresInvalidDisplaysAndUsesCreationForFirstObservation() {
		assertFalse(size.sync(false, 854, 480, false, 0, 0, noRepair));
		assertFalse(size.sync(true, 0, 480, false, 0, 0, noRepair));
		assertFalse(size.sync(true, 854, 0, false, 0, 0, noRepair));
		assertFalse(size.sync(true, 854, 480, false, 854, 480, noRepair));
		assertFalse(size.sync(true, 0, 0, false, 854, 480, noRepair));
		assertFalse(size.sync(true, 854, 480, false, 854, 480, noRepair));
		assertTrue(size.sync(true, 854, 600, false, 854, 600, noRepair));
		assertFalse(size.sync(true, 854, 600, false, 854, 600, noRepair));
		assertTrue(size.sync(true, 1024, 600, false, 1024, 600, noRepair));
		assertFalse(size.sync(false, 1024, 600, false, 0, 0, noRepair));
		assertFalse(size.sync(true, 1920, 1080, false, 1920, 1080, noRepair));
	}

	@Test
	void mismatchRepairStillSignalsRecoveryWithoutDisplayChange() {
		assertFalse(size.sync(true, 854, 480, false, 854, 480, noRepair));
		int[] repaired = new int[2];
		assertTrue(size.sync(true, 854, 480, false, 640, 360, (w, h) -> {
			repaired[0] = w;
			repaired[1] = h;
		}));
		assertArrayEquals(new int[] {854, 480}, repaired);
		assertFalse(size.sync(true, 854, 480, false, 854, 480, noRepair));
	}

	@Test
	void fullscreenSkipsRepairButStillTracksSizeChanges() {
		assertFalse(size.sync(true, 854, 480, true, 640, 360, noRepair));
		assertTrue(size.sync(true, 1920, 1080, true, 640, 360, noRepair));
		assertFalse(size.sync(true, 1920, 1080, true, 640, 360, noRepair));
	}
}
