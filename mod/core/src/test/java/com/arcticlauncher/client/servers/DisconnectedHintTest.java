package com.arcticlauncher.client.servers;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.account.InvalidSession;
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

	@Test
	void recognizesInvalidSessionInPlainFormattedAndTranslatedReasons() {
		assertTrue(InvalidSession.matches("Invalid session"));
		assertTrue(InvalidSession.matches("disconnect.loginFailedInfo.invalidSession"));
		assertTrue(InvalidSession.matches("Failed to login: §cInvalid §rsession (Try restarting your game)"));
		assertTrue(InvalidSession.matches("Giriş başarısız: Geçersiz oturum", "Geçersiz oturum"));
		assertTrue(InvalidSession.matches("Invalid credentials"));
		assertFalse(InvalidSession.matches("Session server unavailable"));
		assertFalse(InvalidSession.matches(null));
	}

	@Test
	void offlineAccountsOnlyAutoReconnectAfterOrdinaryDisconnects() {
		assertFalse(DisconnectedPage.allowsAutoReconnect(InvalidSession.matches("Invalid session"), () -> true));
		assertTrue(DisconnectedPage.allowsAutoReconnect(InvalidSession.matches("You are not whitelisted"), () -> true));
		assertTrue(DisconnectedPage.allowsAutoReconnect(InvalidSession.matches("Timed out"), () -> true));
		assertTrue(DisconnectedPage.allowsAutoReconnect(InvalidSession.matches("invalidSession"), () -> false));
	}

	@Test
	void invalidSessionHintsExplainSwitchingAccountsOrSigningInAgain() {
		Lang.init(null, "en");
		assertTrue(DisconnectedPage.sessionHint(true, true, false).contains("Microsoft account"));
		assertTrue(DisconnectedPage.sessionHint(true, true, false).contains("Reconnect"));
		assertTrue(DisconnectedPage.sessionHint(true, false, true).contains("Sign in again"));
		assertNull(DisconnectedPage.sessionHint(true, false, false));
		assertNull(DisconnectedPage.sessionHint(false, true, false));
	}

	@Test
	void aWrappedLineKeepsItsColor() {
		assertEquals("§c", DisconnectedPage.activeCodes("§cPlease connect"));
		assertEquals("§b§l", DisconnectedPage.activeCodes("§cred §bblue §lbold"));
		assertEquals("", DisconnectedPage.activeCodes("§cred§r plain"));
		assertEquals("", DisconnectedPage.activeCodes("no codes"));
	}
}
