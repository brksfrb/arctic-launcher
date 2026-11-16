package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class ChatClearTest {
	@Test
	void theClearIsForcedOnlyWhileItRuns() {
		boolean[] during = new boolean[1];
		assertFalse(ChatClear.forced());
		ChatClear.run(() -> during[0] = ChatClear.forced());
		assertTrue(during[0]);
		assertFalse(ChatClear.forced());
	}

	@Test
	void aFailingClearDoesNotLeaveItForced() {
		assertThrows(IllegalStateException.class, () -> ChatClear.run(() -> {
			throw new IllegalStateException("boom");
		}));
		assertFalse(ChatClear.forced());
	}
}
