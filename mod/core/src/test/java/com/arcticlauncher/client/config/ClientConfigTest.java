package com.arcticlauncher.client.config;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Shared config defaults, migrations and persistence. */
class ClientConfigTest {
	@TempDir Path dir;

	private ClientConfig load(String json) throws Exception {
		Files.write(dir.resolve("arctic.json"), json.getBytes(StandardCharsets.UTF_8));
		return ClientConfig.load(dir.toFile());
	}

	@Test
	void chatCopyDefaultsOnForOldConfigsAndPersistsOff() throws Exception {
		assertTrue(ClientConfig.load(dir.toFile()).chatCopy);
		ClientConfig c = load("{}");
		assertTrue(c.chatCopy);
		c.chatCopy = false;
		assertEquals(null, c.save());
		assertFalse(ClientConfig.load(dir.toFile()).chatCopy);
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
	@Test
	void controllerDefaultsOffWithoutAFile() {
		assertFalse(new ClientConfig().controller.enabled);
		assertFalse(ClientConfig.load(dir.toFile()).controller.enabled);
	}

	@Test
	void oldControllerDefaultMigratesOnceAndMenuChoicesPersist() throws Exception {
		ClientConfig c = load("{\"controller\":{\"enabled\":true,\"lookSpeed\":5,\"cursorSpeed\":5,\"invertY\":false}}");
		assertFalse(c.controller.enabled);
		assertTrue(c.controller.enabledConfigured);
		c.controller.migrateDefaults();
		assertFalse(c.controller.enabled);
		assertEquals(null, c.save());
		c = ClientConfig.load(dir.toFile());
		assertFalse(c.controller.enabled);
		c.controller.setEnabled(true);
		assertEquals(null, c.save());
		c = ClientConfig.load(dir.toFile());
		assertTrue(c.controller.enabled);
		c.controller.migrateDefaults();
		assertTrue(c.controller.enabled);
		c.controller.setEnabled(false);
		assertEquals(null, c.save());
		assertFalse(ClientConfig.load(dir.toFile()).controller.enabled);
	}

	@Test
	void oldDisabledControllerStaysOffEvenWithCustomSpeeds() throws Exception {
		for (String extra : new String[] {"", ",\"lookSpeed\":8"}) {
			ClientConfig c = load("{\"controller\":{\"enabled\":false" + extra + "}}");
			assertFalse(c.controller.enabled);
			assertEquals(null, c.save());
			assertFalse(ClientConfig.load(dir.toFile()).controller.enabled);
		}
	}

	@Test
	void eachOldControllerCustomizationPreservesEnabledAcrossRestarts() throws Exception {
		for (String custom : new String[] {"\"lookSpeed\":6", "\"cursorSpeed\":4", "\"invertY\":true"}) {
			ClientConfig c = load("{\"controller\":{\"enabled\":true," + custom + "}}");
			assertTrue(c.controller.enabled);
			assertTrue(c.controller.enabledConfigured);
			assertEquals(null, c.save());
			c = ClientConfig.load(dir.toFile());
			assertTrue(c.controller.enabled);
			// A migrated user may later restore factory speeds without losing their choice.
			c.controller.lookSpeed = 5;
			c.controller.cursorSpeed = 5;
			c.controller.invertY = false;
			assertEquals(null, c.save());
			assertTrue(ClientConfig.load(dir.toFile()).controller.enabled);
		}
	}


	@Test
	void toggleKeysLoadEmptyFromAnOldFileAndPersist() throws Exception {
		assertTrue(load("{}").toggleKeys.isEmpty());
		assertTrue(load("{\"toggleKeys\":null}").toggleKeys.isEmpty());
		ClientConfig c = load("{}");
		c.toggleKeys.put("connected-glass", "key.keyboard.j");
		assertEquals(null, c.save());
		assertEquals("key.keyboard.j", ClientConfig.load(dir.toFile()).toggleKeys.get("connected-glass"));
	}
}
