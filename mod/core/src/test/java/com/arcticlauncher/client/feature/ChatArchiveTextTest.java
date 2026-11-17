package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.*;
import org.junit.jupiter.api.Test;

class ChatArchiveTextTest {
	/** What the modern ChatHistoryText writes for the FAWE component (font minecraft:default, hex colours). */
	private static final String FAWE = "§r§@minecraft:default;§#555555(§r§@minecraft:default;§#aa0000FAWE"
			+ "§r§@minecraft:default;§#555555)§r§@minecraft:default;§#aaaaaa 8 blocks moved";

	/** A legacy renderer's visible text: every code (section sign + one char) removed. */
	private static String visible(String s) { return s.replaceAll("§.", ""); }

	@Test void faweLineLosesFontAndHex() {
		String out = ChatArchiveText.toLegacy(FAWE);
		assertEquals("(FAWE) 8 blocks moved", visible(out));
		assertFalse(out.contains("§@") || out.contains("§#") || out.contains("minecraft:default"));
		assertEquals("§r§8(§r§4FAWE§r§8)§r§7 8 blocks moved", out);
	}

	@Test void legacyLinesAreUntouched() {
		String s = "§a§lHello §r§7world § end§";
		assertSame(s, ChatArchiveText.toLegacy(s));
		assertNull(ChatArchiveText.toLegacy(null));
	}

	@Test void malformedMarkersNeverLeakText() {
		assertEquals("BAD:font text", ChatArchiveText.toLegacy("§@BAD:font text"));
		assertEquals("zz1", ChatArchiveText.toLegacy("§#zz1"));
		assertEquals("x§r", ChatArchiveText.toLegacy("§@a:b;x§r"));
		assertEquals("a§cb", ChatArchiveText.toLegacy("a§#ff5555b"));
	}

	@Test void nearestColour() {
		assertEquals(0xc, ChatArchiveText.nearest(0xff5050));
		assertEquals(0xf, ChatArchiveText.nearest(0xfefefe));
		assertEquals(0x0, ChatArchiveText.nearest(0x010101));
	}
}
