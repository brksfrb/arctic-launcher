package com.arcticlauncher.client.items;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class HeldItemTest {
	private static final long SECOND = 1_000_000_000L;

	private static final class Item implements HeldItem.Track {
		double airY;
		long airAt;

		@Override
		public double arctic$airY() {
			return airY;
		}

		@Override
		public long arctic$airAt() {
			return airAt;
		}

		@Override
		public void arctic$setAir(double y, long at) {
			airY = y;
			airAt = at;
		}
	}

	@Test
	void aFallingItemIsNeverHeld() {
		Item item = new Item();
		double y = 80;
		for (long t = 1; t <= 4 * SECOND; t += SECOND / 20) {
			assertFalse(HeldItem.held(item, false, false, y, t), "at " + t);
			y -= 0.2;
		}
	}

	@Test
	void anItemTheServerKeepsPuttingBackIsHeldAfterAMoment() {
		Item item = new Item();
		long t = 1;
		// The game lets it fall a little, the server puts it back: it never gets a block lower.
		for (; t < SECOND; t += SECOND / 20) {
			assertFalse(HeldItem.held(item, false, false, 64.5 - (t % (SECOND / 4)) * 1e-9, t));
		}
		boolean held = false;
		for (; t < 3 * SECOND; t += SECOND / 20) {
			held = HeldItem.held(item, false, false, 64.5 - (t % (SECOND / 4)) * 1e-9, t);
		}
		assertTrue(held);
	}

	@Test
	void anItemWithoutGravityIsHeldAtOnceAndLandingStartsOver() {
		Item item = new Item();
		assertTrue(HeldItem.held(item, false, true, 70, 1));
		assertFalse(HeldItem.held(item, true, true, 70, 2));
		assertFalse(HeldItem.held(item, true, false, 70, 3 * SECOND));
		// Knocked off again: a fresh start, not held from the time before.
		assertFalse(HeldItem.held(item, false, false, 70, 4 * SECOND));
	}

	@Test
	void anItemThrownUpComesDownAsAFall() {
		Item item = new Item();
		long t = 1;
		double y = 64;
		double speed = 0.4;
		for (; t < 2 * SECOND; t += SECOND / 20) {
			assertFalse(HeldItem.held(item, false, false, y, t), "at " + t);
			y += speed;
			speed -= 0.04;
		}
	}
}
