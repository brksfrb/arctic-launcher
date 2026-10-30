package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class StreamerTest {
	@Test
	void aNameMatchesWhereverItSits() {
		assertEquals(14, Streamer.matchEnd("name: brksfrb3", 6, "brksfrb3"));
		assertEquals(-1, Streamer.matchEnd("name: brksfrb4", 6, "brksfrb3"));
	}

	@Test
	void colourCodesInsideTheNameDoNotHideIt() {
		// A long scoreboard line is split into prefix and suffix with the team's codes in between.
		String line = "name: §abrksf§1§arb3";
		assertEquals(line.length(), Streamer.matchEnd(line, 8, "brksfrb3"));
	}

	@Test
	void aCodeBeforeTheNameIsNotPartOfTheMatch() {
		assertEquals(-1, Streamer.matchEnd("§abrksfrb3", 0, "brksfrb3"));
	}
}
