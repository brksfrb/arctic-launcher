package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;

class ChatStackerTest {
	@Test void blankLinesNeverStackEvenWhenTheSettingIsOff() {
		ClientConfig c = new ClientConfig();
		c.chatStackIgnoreBlank = false;
		ChatStacker s = new ChatStacker();
		String[] blanks = {"", " ", "   ", "\u00a7r", "\u00a7f \u00a7r", "\t", "\u00a0", "\u200b", "\u00a7"};
		for (String b : blanks) {
			s.reset();
			for (int i = 0; i < 53; i++) {
				assertEquals(1, s.next(b, c), "blank " + b.length() + " stacked");
			}
			assertTrue(ChatStacker.isBlank(b));
		}
	}

	@Test void realLinesStillStack() {
		ClientConfig c = new ClientConfig();
		ChatStacker s = new ChatStacker();
		assertEquals(1, s.next("hello", c));
		assertEquals(2, s.next("hello", c));
		assertEquals(3, s.next("hello", c));
		assertFalse(ChatStacker.isBlank("\u00a7ahi"));
		assertFalse(ChatStacker.isBlank("."));
		assertFalse(ChatStacker.isBlank("\u00a7a-"));
	}
}
