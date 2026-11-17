package com.arcticlauncher.client.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class RideableHudTest {
	/** {max health, jump bar 0/1, xp progress, level}. */
	private static final float[] HORSE = {30f, 1f, 0.5f, 7f};
	private static final float[] STRIDER = {20f, 0f, 0.5f, 7f};

	@Test
	void offOrNotMountedShowsNothing() {
		assertFalse(RideableHud.showHunger(false, HORSE));
		assertFalse(RideableHud.showXp(false, HORSE));
		assertFalse(RideableHud.showHunger(true, null));
		assertFalse(RideableHud.showXp(true, null));
	}

	@Test
	void jumpingMountsGetHungerAndXp() {
		assertTrue(RideableHud.showHunger(true, HORSE));
		assertTrue(RideableHud.showXp(true, HORSE));
	}

	@Test
	void mountsWithoutJumpBarKeepTheGamesXpSoOnlyHungerIsAdded() {
		assertTrue(RideableHud.showHunger(true, STRIDER));
		assertFalse(RideableHud.showXp(true, STRIDER));
	}

	@Test
	void hungerSitsAboveEveryRowOfMountHearts() {
		assertEquals(1, RideableHud.mountRows(20f));
		assertEquals(2, RideableHud.mountRows(30f));
		assertEquals(2, RideableHud.mountRows(21f));
		assertEquals(49, RideableHud.hungerAbove(20f));
		assertEquals(59, RideableHud.hungerAbove(30f));
		assertEquals(49, RideableHud.hungerAbove(0f));
	}
}
