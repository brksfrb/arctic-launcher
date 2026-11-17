package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.config.ClientConfig;
import com.google.gson.JsonParser;
import java.io.InputStreamReader;
import java.util.HashSet;
import java.util.Set;
import org.junit.jupiter.api.Test;

class EmoteAtlasTest {
	@Test void exactlyApprovedCatalogAndUvs() {
		com.google.gson.JsonArray sprites = new JsonParser().parse(new InputStreamReader(
				getClass().getResourceAsStream("/assets/arctic/emotes.json"))).getAsJsonObject().getAsJsonArray("sprites");
		assertEquals(64, EmoteAtlas.all().size());
		assertEquals(64, sprites.size());
		Set<String> codes = new HashSet<String>();
		for (int i = 0; i < 64; i++) {
			EmoteAtlas.Emote e = EmoteAtlas.all().get(i);
			com.google.gson.JsonObject expected = sprites.get(i).getAsJsonObject();
			assertTrue(codes.add(e.code));
			assertEquals(expected.get("code").getAsString(), e.code);
			assertEquals(expected.get("x").getAsInt(), e.u() + 8);
			assertEquals(expected.get("y").getAsInt(), e.v() + 8);
			assertSame(e, EmoteAtlas.glyph(e.glyph()));
			for (com.google.gson.JsonElement alias : expected.getAsJsonArray("aliases")) {
				assertSame(e, EmoteAtlas.named(alias.getAsString()));
				assertEquals(String.valueOf(e.glyph()), EmoteAtlas.layout(":" + alias.getAsString() + ":", true));
				assertEquals(":" + alias.getAsString() + ":", Emoji.expand(":" + alias.getAsString() + ":"));
			}
			assertEquals(e.shortcode(), Emoji.expand(e.shortcode()));
		}
	}
	@Test void wirePreservesAtlasAndUnknownButExpandsOldNonConflictingCodes() {
		assertEquals(":heart:peace:", Emoji.expand(":heart:peace:"));
		String input = ":heart::star::smile::sad::skull::snow::snowflake::snowman::swords::check::x::cross::crown:";
		assertEquals(input, Emoji.expand(input));
		assertEquals(":SOB: :unknown: ☮ ☕", Emoji.expand(":SOB: :unknown: :peace: :coffee:"));
	}
	@Test void typedWireAndToggleOff() {
		ClientConfig c = new ClientConfig();
		assertEquals(":sob::heart::iceberg: ☮", QuickMessages.fillTyped(":sob::heart::iceberg: :peace:", c, null));
		c.chatEmoji = false;
		assertEquals(":sob::heart::peace:", QuickMessages.fillTyped(":sob::heart::peace:", c, null));
	}

	@Test void parsingAdjacentOverlappingUnknownCaseAndBounds() {
		String sob = String.valueOf(EmoteAtlas.named("sob").glyph());
		assertEquals(sob + sob, EmoteAtlas.layout(":sob::SOB:", true));
		assertEquals(sob + "sob:", EmoteAtlas.layout(":sob:sob:", true));
		assertEquals(":unknown:" + sob, EmoteAtlas.layout(":unknown::sob:", true));
		assertEquals(":" + sob, EmoteAtlas.layout("::sob:", true));
		assertEquals("word:sob : :missing", EmoteAtlas.layout("word:sob : :missing", true));
		String longAlias = ":stuck_out_tongue_winking:";
		assertTrue(longAlias.length() > 22);
		assertEquals(String.valueOf(EmoteAtlas.named("tongue").glyph()), EmoteAtlas.layout(longAlias, true));
		assertEquals(":abcdefghijklmnopqrstuvwxyz0123456789:", EmoteAtlas.layout(":abcdefghijklmnopqrstuvwxyz0123456789:", true));
	}
	@Test void distinctIceAndAliases() {
		assertSame(EmoteAtlas.named("ice_cube"), EmoteAtlas.named("iceberg"));
		assertNotSame(EmoteAtlas.named("ice"), EmoteAtlas.named("iceberg"));
		assertEquals("svg/1f43b-200d-2744-fe0f.svg", new JsonParser().parse(new InputStreamReader(
				getClass().getResourceAsStream("/assets/arctic/emotes.json"))).getAsJsonObject().getAsJsonArray("sprites").get(63).getAsJsonObject().get("file").getAsString());
	}
	@Test void disabledLayoutAndEveryPickerCell() {
		assertTrue(new ClientConfig().chatEmoji);
		assertEquals(":sob::peace:", EmoteAtlas.layout(":sob::peace:", false));
		ClientConfig config = new ClientConfig();
		EmojiPicker.closed();
		assertEquals("", EmojiPicker.click(config, 630, 349, 640, 360));
		assertEquals(64, EmojiPicker.layout().size());
		for (EmojiPicker.Cell cell : EmojiPicker.layout()) {
			int x = 640 - EmojiPicker.panelWidth() - 4 + cell.x + 2;
			int y = 360 - 11 - 3 - EmojiPicker.panelHeight() - 4 + cell.y + 2;
			assertEquals(cell.emote.shortcode(), EmojiPicker.click(config, x, y, 640, 360));
		}
		EmojiPicker.closed();
	}
}
