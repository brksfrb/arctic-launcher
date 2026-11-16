package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;

class MenuBlurTest {
	@Test
	void theGamesOwnStrengthStaysWhileOff() {
		ClientConfig c = new ClientConfig();
		assertEquals(3, MenuBlur.strength(c, 3));
		assertEquals(3, MenuBlur.strength(null, 3));
	}

	@Test
	void ourStrengthReplacesItWhileOnAndStaysInRange() {
		ClientConfig c = new ClientConfig();
		c.menuBlur = true;
		c.menuBlurStrength = 7;
		assertEquals(7, MenuBlur.strength(c, 0));
		c.menuBlurStrength = 99;
		assertEquals(MenuBlur.MAX, MenuBlur.strength(c, 0));
		c.menuBlurStrength = -4;
		assertEquals(0, MenuBlur.strength(c, 5));
	}
}
