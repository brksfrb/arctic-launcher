package com.arcticlauncher.client.config;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Combat Feedback's tile is new; older arctic.json files only had the sub-options. */
class ClientConfigTest {
	@TempDir Path dir;

	private ClientConfig load(String json) throws Exception {
		Files.write(dir.resolve("arctic.json"), json.getBytes(StandardCharsets.UTF_8));
		return ClientConfig.load(dir.toFile());
	}

	@Test
	void aNewConfigStartsWithTheTileOff() {
		ClientConfig c = ClientConfig.load(dir.toFile());
		assertFalse(c.combatFeedback);
	}

	@Test
	void anOldFileTurnsTheTileOnWhenAnyPartWasInUse() throws Exception {
		assertTrue(load("{\"hitMarker\":true}").combatFeedback);
		assertTrue(load("{\"hitSound\":2}").combatFeedback);
		assertTrue(load("{\"killSound\":1}").combatFeedback);
		assertTrue(load("{\"lowHealthWarning\":true}").combatFeedback);
	}

	@Test
	void anOldFileWithNothingOnLeavesTheTileOff() throws Exception {
		assertFalse(load("{}").combatFeedback);
	}

	@Test
	void aSavedOffTileStaysOffEvenIfASubOptionIsOn() throws Exception {
		ClientConfig c = load("{\"combatFeedback\":false,\"hitMarker\":true,\"hitSound\":3,\"lowHealthWarning\":true}");
		assertFalse(c.combatFeedback);
		assertTrue(c.hitMarker);
	}

	@Test
	void migrationIsWrittenSoTheNextLoadKeepsAnExplicitOff() throws Exception {
		ClientConfig c = load("{\"hitSound\":1}");
		assertTrue(c.combatFeedback);
		c.combatFeedback = false;
		assertEquals(null, c.save());
		ClientConfig again = ClientConfig.load(dir.toFile());
		assertFalse(again.combatFeedback);
		assertEquals(1, again.hitSound);
	}
}
