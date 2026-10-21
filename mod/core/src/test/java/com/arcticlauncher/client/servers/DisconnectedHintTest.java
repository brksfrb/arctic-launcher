package com.arcticlauncher.client.servers;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class DisconnectedHintTest {
	@Test
	void aBareDisconnectedIsExplained() {
		assertNotNull(DisconnectedPage.hint("Disconnected"));
		assertNotNull(DisconnectedPage.hint(""));
		assertTrue(DisconnectedPage.hint("Disconnected").contains("without saying why"));
	}

	@Test
	void technicalReasonsSaySimplyWhatHappened() {
		assertTrue(DisconnectedPage.hint("Internal Exception: io.netty.handler.timeout.ReadTimeoutException").contains("didn't answer in time"));
		assertTrue(DisconnectedPage.hint("Connection refused: getsockopt").contains("offline"));
		assertTrue(DisconnectedPage.hint("java.net.UnknownHostException: play.example.invalid").contains("doesn't exist"));
		assertTrue(DisconnectedPage.hint("Connection reset").contains("cut"));
	}

	@Test
	void aReasonFromTheServerIsLeftAlone() {
		assertNull(DisconnectedPage.hint("You are banned from this server."));
		assertNull(DisconnectedPage.hint("You are not whitelisted on this server!"));
		assertNull(DisconnectedPage.hint("Outdated client! Please use 1.21.4"));
	}
}
