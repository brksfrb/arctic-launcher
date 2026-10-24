package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;

class DynamicFpsTest {
	private static ClientConfig config() {
		ClientConfig c = new ClientConfig();
		c.dynamicFps = true;
		c.dynFpsUnfocused = 15;
		c.dynFpsIdle = 30;
		c.dynFpsIdleSeconds = 60;
		c.dynFpsMenu = 60;
		return c;
	}

	@Test
	void nothingChangesWhileTheGameIsInUse() {
		assertEquals(144, DynamicFps.apply(144, true, false, 5_000, config()));
	}

	@Test
	void anotherWindowDropsTheLimit() {
		assertEquals(15, DynamicFps.apply(144, false, false, 0, config()));
	}

	@Test
	void noLimitInTheGameBecomesTheUnfocusedOne() {
		assertEquals(15, DynamicFps.apply(0, false, false, 0, config()));
		assertEquals(0, DynamicFps.apply(0, true, false, 0, config()));
	}

	@Test
	void idleOnlyAfterTheSetTime() {
		assertEquals(144, DynamicFps.apply(144, true, false, 59_000, config()));
		assertEquals(30, DynamicFps.apply(144, true, false, 60_000, config()));
	}

	@Test
	void menusLimitTheGameToTheMenuRate() {
		assertEquals(60, DynamicFps.apply(144, true, true, 0, config()));
	}

	@Test
	void theLowestLimitThatAppliesWins() {
		assertEquals(30, DynamicFps.apply(144, true, true, 120_000, config()));
	}

	@Test
	void theGamesOwnLowerLimitIsKept() {
		assertEquals(10, DynamicFps.apply(10, false, false, 0, config()));
		assertEquals(30, DynamicFps.apply(30, true, true, 0, config()));
	}

	@Test
	void aZeroSettingTurnsThatLimitOff() {
		ClientConfig c = config();
		c.dynFpsUnfocused = 0;
		assertEquals(144, DynamicFps.apply(144, false, false, 0, c));
	}

	@Test
	void neverBelowTheFloor() {
		ClientConfig c = config();
		c.dynFpsUnfocused = 1;
		assertEquals(5, DynamicFps.apply(144, false, false, 0, c));
	}
}
