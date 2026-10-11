package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonParser;
import java.util.Map;
import org.junit.jupiter.api.Test;

class FitTest {
	@SuppressWarnings("deprecation")
	private static Map<String, Fit> parse(String json) {
		return Fit.parse(new JsonParser().parse(json));
	}

	@Test
	void readsMovesAndSides() {
		Map<String, Fit> fits = parse("{\"visor\": {\"up\": 2, \"forward\": -1}, \"fox\": {\"mirror\": true}}");

		Fit visor = fits.get("visor");
		assertEquals(-1f, visor.dy(), 1e-6);
		assertEquals(0.5f, visor.dz(), 1e-6);
		assertFalse(visor.mirror);
		assertTrue(fits.get("fox").mirror);
	}

	@Test
	void staysWithinTheServersLimits() {
		Fit f = parse("{\"visor\": {\"up\": 99, \"forward\": -99}}").get("visor");

		assertEquals(Fit.MAX_UP, f.up);
		assertEquals(-Fit.MAX_FORWARD, f.forward);
	}

	@Test
	void brokenOrEmptyFitsAreNone() {
		assertTrue(parse("{\"a\": 5, \"b\": {\"up\": \"x\"}, \"c\": {}}").isEmpty());
		assertTrue(parse("[]").isEmpty());
		Look look = new Look(null, false, null, java.util.Collections.<String>emptyList(), true, 0);
		assertSame(Fit.NONE, look.fit("anything"));
	}
}
