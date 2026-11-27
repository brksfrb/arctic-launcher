package com.arcticlauncher.legacy;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class LegacyScrollTest {
	@Test
	void oneNotchIsTwoModernUnitsOnEveryPlatform() {
		assertEquals(0, LegacyScroll.amount(0));
		assertEquals(2, LegacyScroll.amount(120));
		assertEquals(-2, LegacyScroll.amount(-120));
		assertEquals(2, LegacyScroll.amount(1));
		assertEquals(-2, LegacyScroll.amount(-1));
	}

	@Test
	void keepsCombinedWindowsNotches() {
		assertEquals(6, LegacyScroll.amount(360));
		assertEquals(-4, LegacyScroll.amount(-240));
		assertEquals(3, LegacyScroll.amount(180));
	}
}
