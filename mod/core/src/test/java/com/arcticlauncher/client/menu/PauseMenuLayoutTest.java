package com.arcticlauncher.client.menu;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class PauseMenuLayoutTest {
	@Test
	void addsARowBelowTheMenuWithOrWithoutIcons() {
		PauseMenuLayout withoutIcons = new PauseMenuLayout(218, 204, 20, 360);
		withoutIcons.avoid(218, 170, 204, 20);
		assertEquals(194, withoutIcons.findY(98, 190));

		PauseMenuLayout withIcons = new PauseMenuLayout(218, 204, 20, 360);
		for (int x = 274; x < 366; x += 24) {
			withIcons.avoid(x, 146, 20, 20);
		}
		withIcons.avoid(218, 170, 98, 20);
		withIcons.avoid(218, 194, 98, 20);
		assertEquals(218, withIcons.findY(98, 214));
	}

	@Test
	void skipsAnAddedButtonInTheMenuColumn() {
		PauseMenuLayout layout = new PauseMenuLayout(218, 204, 20, 360);
		layout.avoid(324, 218, 98, 20);
		layout.avoid(426, 242, 98, 20);
		assertEquals(242, layout.findY(98, 214));
	}

	@Test
	void usesAGapWhenTheMenuReachesTheBottom() {
		PauseMenuLayout layout = new PauseMenuLayout(218, 204, 20, 240);
		layout.avoid(218, 146, 204, 20);
		layout.avoid(218, 194, 204, 20);
		assertEquals(170, layout.findY(146, 214));
	}
}
