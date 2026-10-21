package com.arcticlauncher.client.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class FrameStatsTest {
	private static final long MS = 1_000_000L;

	@BeforeEach
	void clear() {
		FrameStats.reset();
	}

	private static void frames(int n, long ms, long[] clock) {
		for (int i = 0; i < n; i++) {
			clock[0] += ms * MS;
			FrameStats.tick(clock[0]);
		}
	}

	@Test
	void unknownUntilThereAreEnoughFrames() {
		long[] clock = {1};
		FrameStats.tick(clock[0]);
		frames(5, 10, clock);
		assertEquals(-1, FrameStats.averageMs());
		assertEquals(-1, FrameStats.lowFps());
	}

	@Test
	void steadyFramesGiveTheirTimeAndRate() {
		long[] clock = {1};
		FrameStats.tick(clock[0]);
		frames(200, 10, clock);
		assertEquals(10.0, FrameStats.averageMs(), 0.01);
		assertEquals(100.0, FrameStats.lowFps(), 0.5);
	}

	@Test
	void aStutterShowsInTheLowButBarelyInTheAverage() {
		long[] clock = {1};
		FrameStats.tick(clock[0]);
		frames(495, 5, clock);
		frames(5, 100, clock);
		assertTrue(FrameStats.averageMs() < 7.5, "average " + FrameStats.averageMs());
		assertEquals(10.0, FrameStats.lowFps(), 0.5);
	}

	@Test
	void aPauseIsNotAFrame() {
		long[] clock = {1};
		FrameStats.tick(clock[0]);
		frames(100, 10, clock);
		clock[0] += 30_000 * MS;
		FrameStats.tick(clock[0]);
		frames(100, 10, clock);
		assertEquals(10.0, FrameStats.averageMs(), 0.01);
	}
}
