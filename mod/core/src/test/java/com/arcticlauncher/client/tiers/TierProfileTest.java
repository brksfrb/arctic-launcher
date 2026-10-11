package com.arcticlauncher.client.tiers;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import org.junit.jupiter.api.Test;

class TierProfileTest {
	/** Trimmed from a real PvPTiers answer. */
	private static final String PROFILE = "{\"uuid\":\"06ec3577329945fabbdf613b1f86c8ab\",\"name\":\"ItzRealMe\",\"rankings\":{"
			+ "\"crystal\":{\"tier\":1,\"pos\":0,\"peak_tier\":1,\"peak_pos\":0,\"attained\":1729201725,\"retired\":true},"
			+ "\"axe\":{\"tier\":2,\"pos\":1,\"peak_tier\":2,\"peak_pos\":1,\"attained\":1739125934,\"retired\":false},"
			+ "\"sword\":{\"tier\":2,\"pos\":0,\"peak_tier\":null,\"peak_pos\":null,\"attained\":1707879318,\"retired\":false},"
			+ "\"broken\":{\"tier\":null}},\"region\":\"NA\",\"points\":300,\"overall\":1}";

	@Test
	void readsEveryRankedMode() {
		TierProfile p = TierProfile.parse(PROFILE);

		assertEquals(3, p.ranks.size());
		assertEquals("RHT1", p.in("crystal").label());
		assertEquals("LT2", p.in("axe").label());
		assertNull(p.in("mace"));
	}

	@Test
	void bestPrefersCurrentTiersThenPoints() {
		TierProfile p = TierProfile.parse(PROFILE);

		// Retired HT1 crystal loses to a current tier; HT2 (30) beats LT2 (20).
		assertEquals("sword", p.best().mode);
		assertEquals(30, p.best().points());
	}

	@Test
	void errorsAreNoProfile() {
		assertNull(TierProfile.parse("{\"code\":\"error.not_found\",\"message\":null}"));
		assertNull(TierProfile.parse("<html>502</html>"));
		assertNull(TierProfile.parse("[]"));
	}

	@Test
	void tagsUseTheTierListColors() {
		TierProfile p = TierProfile.parse(PROFILE);

		assertEquals(0xC4D3E7, Tiers.color(p.in("sword")));
		assertEquals('f', Tiers.legacy(p.in("sword")));
		assertEquals('b', Tiers.legacy(p.in("crystal")));
		assertEquals("4b2d4cd2e7e94f5e8bd3c0e0b19a1a6f", Tiers.idIn("{\"id\":\"4b2d4cd2e7e94f5e8bd3c0e0b19a1a6f\",\"name\":\"x\"}"));
		assertEquals("", Tiers.idIn("not json"));
	}
}
