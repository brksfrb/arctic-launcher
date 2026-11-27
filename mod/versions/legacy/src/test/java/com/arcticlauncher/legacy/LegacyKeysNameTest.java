package com.arcticlauncher.legacy;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;
import org.lwjgl.input.Keyboard;

class LegacyKeysNameTest {
	@Test
	void outOfRangeKeyboardCodesAreUnknown() {
		assertEquals("key.keyboard.unknown", LegacyKeys.name(Keyboard.KEYBOARD_SIZE));
		assertEquals("key.keyboard.unknown", LegacyKeys.name(256));
		assertEquals("key.keyboard.unknown", LegacyKeys.name(Integer.MAX_VALUE));
		assertEquals("key.keyboard.unknown", LegacyKeys.name(0));
	}

	@Test
	void validCodesStillNamed() {
		assertEquals("key.keyboard.c", LegacyKeys.name(Keyboard.KEY_C));
		assertEquals("key.mouse.left", LegacyKeys.name(-100));
	}
}
