package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class SessionLineTest {
	private static final String JWT = "eyJraWQiOiJhYzg0YSIsImFsZyI6IkhTMjU2In0.eyJ4dWlkIjoiMjUzNTQyIn0.c2lnbmF0dXJlLXBhcnQtaGVyZQ";
	private static final String ID = "593fd4c3951f39ada7bd19ddba5fcb41";

	@Test
	void theStartUpLineLosesItsTokenAndPlayerId() {
		String line = "(Session ID is token:" + JWT + ":" + ID + ")";
		assertTrue(SessionLine.secret(line));
		assertEquals("(Session ID is <hidden>)", SessionLine.mask(line));
	}

	@Test
	void anOldStyleTokenIsHiddenToo() {
		assertEquals("(Session ID is <hidden>)", SessionLine.mask("(Session ID is token:0123456789abcdef0123456789abcdef:" + ID + ")"));
	}

	@Test
	void anOfflinePlayersLineIsHiddenAsWell() {
		// Nothing secret in it, but one rule is easier to trust than two.
		assertEquals("(Session ID is <hidden>)", SessionLine.mask("(Session ID is token:0:" + ID + ")"));
	}

	@Test
	void aTokenAnywhereElseIsHidden() {
		String text = "Connecting with token:" + JWT + ":" + ID + " to the server\nnext line";
		String masked = SessionLine.mask(text);
		assertEquals("Connecting with token:<hidden> to the server\nnext line", masked);
		assertFalse(masked.contains("eyJ"));
	}

	@Test
	void theRestOfALongTextStays() {
		String text = "before\n[Client thread/INFO]: (Session ID is token:" + JWT + ":" + ID + ")\nafter (and more)";
		assertEquals("before\n[Client thread/INFO]: (Session ID is <hidden>)\nafter (and more)", SessionLine.mask(text));
	}

	@Test
	void ordinaryLinesAreLeftAlone() {
		for (String line : new String[] {"Setting user: Steve", "a token: of thanks", "token:short", "", "(Session ID is <hidden>)"}) {
			assertFalse(SessionLine.secret(line), line);
			assertSame(line, SessionLine.mask(line));
		}
		assertFalse(SessionLine.secret(null));
	}
}
