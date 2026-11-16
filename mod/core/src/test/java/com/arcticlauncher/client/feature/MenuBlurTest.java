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

	@Test
	void frostKeepsTheGamesGradientWhileOffAndFollowsTheStrengthWhileOn() {
		ClientConfig c = new ClientConfig();
		assertEquals(0xC0101010, MenuBlur.frost(c, 0xC0101010));
		c.menuBlur = true;
		c.menuBlurStrength = 0;
		assertEquals(0x00, MenuBlur.frost(c, 0xC0101010) >>> 24);
		c.menuBlurStrength = 5;
		int top = MenuBlur.frost(c, 0xC0101010) >>> 24;
		int bottom = MenuBlur.frost(c, 0xD0101010) >>> 24;
		assertEquals(110, top);
		assertEquals(126, bottom);
		c.menuBlurStrength = 10;
		assertEquals(220, MenuBlur.frost(c, 0xC0101010) >>> 24);
		assertEquals(0x1A2230, MenuBlur.frost(c, 0xC0101010) & 0xFFFFFF);
	}
}
