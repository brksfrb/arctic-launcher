package com.arcticlauncher.client.together;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.together.OfflineJoin.Verdict;
import java.net.InetAddress;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class OfflineJoinTest {
	private static final List<String> ONLINE = java.util.Arrays.asList("Alex", "Notch");

	private static Verdict decide(boolean enabled, boolean integrated, boolean loopback, boolean verified, String name) {
		return OfflineJoin.decide(enabled, integrated, loopback, verified, name, "HostPlayer", ONLINE);
	}

	@Test
	void settingOffNonLoopbackOrNotIntegratedLeavesTheGameToDecide() {
		assertEquals(Verdict.LEAVE, decide(false, true, true, false, "Friend"));
		assertEquals(Verdict.LEAVE, decide(true, true, false, false, "Friend"));
		assertEquals(Verdict.LEAVE, decide(true, false, true, false, "Friend"));
		// Even a clashing name is the game's business when the feature doesn't apply.
		assertEquals(Verdict.LEAVE, decide(true, true, false, false, "hostplayer"));
	}

	@Test
	void failedSessionOnLoopbackIsAcceptedOffline() {
		assertEquals(Verdict.ACCEPT_OFFLINE, decide(true, true, true, false, "Friend"));
	}

	@Test
	void verifiedSessionStaysOfficial() {
		assertEquals(Verdict.LEAVE, decide(true, true, true, true, "Friend"));
		assertEquals(Verdict.LEAVE, decide(true, true, true, true, "HostPlayer"));
	}

	@Test
	void hostNameIsRejectedInAnyCase() {
		assertEquals(Verdict.SAME_NAME, decide(true, true, true, false, "hostplayer"));
		assertEquals(Verdict.SAME_NAME, decide(true, true, true, false, "HOSTPLAYER"));
	}

	@Test
	void nameOfSomeoneOnlineIsRejected() {
		assertEquals(Verdict.SAME_NAME, decide(true, true, true, false, "alex"));
		assertEquals(Verdict.SAME_NAME, decide(true, true, true, false, "NOTCH"));
	}

	@Test
	void aDifferentNameIsAllowed() {
		assertEquals(Verdict.ACCEPT_OFFLINE, decide(true, true, true, false, "Alexa"));
		assertEquals(Verdict.ACCEPT_OFFLINE, OfflineJoin.decide(true, true, true, false, "Friend", null, null));
	}

	@Test
	void loopbackMeansThisPc() throws Exception {
		assertTrue(OfflineJoin.loopback(InetAddress.getByName("127.0.0.1")));
		assertTrue(OfflineJoin.loopback(InetAddress.getByName("::1")));
		assertFalse(OfflineJoin.loopback(InetAddress.getByName("192.168.1.20")));
		assertFalse(OfflineJoin.loopback(null));
	}

	@Test
	void offlineIdMatchesVanillaOfflineMode() {
		assertEquals(UUID.fromString("b50ad385-829d-3141-a216-7e7d7539ba7f"), OfflineJoin.offlineId("Notch"));
	}

	@Test
	void guestSkipsTheMojangCallOnlyOfflineAndOnThisPc() {
		assertTrue(OfflineJoin.skipJoinAuth("0", true));
		assertFalse(OfflineJoin.skipJoinAuth("0", false));
		assertFalse(OfflineJoin.skipJoinAuth("eyJtoken", true));
		assertFalse(OfflineJoin.skipJoinAuth(null, true));
	}
}
