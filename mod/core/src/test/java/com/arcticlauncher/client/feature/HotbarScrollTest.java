package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HotbarScrollTest {
	@Test
	void wrappingMatchesVanillaOneNotch() {
		assertEquals(8, HotbarScroll.wheel(0, 1, true));
		assertEquals(0, HotbarScroll.wheel(8, -1, true));
		assertEquals(2, HotbarScroll.wheel(3, 1, true));
		assertEquals(4, HotbarScroll.wheel(3, -1, true));
		assertEquals(3, HotbarScroll.wheel(3, 0, true));
	}

	/** A large wheel delta is still one slot: the game keeps only the sign. */
	@Test
	void wheelUsesTheSignOnly() {
		assertEquals(8, HotbarScroll.wheel(0, 5.5, true));
		assertEquals(0, HotbarScroll.wheel(8, -9, true));
		assertEquals(0, HotbarScroll.wheel(0, 5.5, false));
		assertEquals(8, HotbarScroll.wheel(8, -9, false));
	}

	@Test
	void clampedScrollStopsOnSlot1AndSlot9() {
		assertEquals(0, HotbarScroll.wheel(0, 1, false));
		assertEquals(0, HotbarScroll.wheel(0, 1, false));
		assertEquals(8, HotbarScroll.wheel(8, -1, false));
		assertEquals(8, HotbarScroll.wheel(8, -1, false));
		assertEquals(2, HotbarScroll.wheel(3, 1, false));
		assertEquals(4, HotbarScroll.wheel(3, -1, false));
		assertEquals(7, HotbarScroll.wheel(8, 1, false));
		assertEquals(1, HotbarScroll.wheel(0, -1, false));
	}

	/** Controller shoulders pass a step, positive toward slot 9. */
	@Test
	void stepStopsAtTheEndsAndWrapsOtherwise() {
		assertEquals(8, HotbarScroll.move(8, 1, false));
		assertEquals(0, HotbarScroll.move(0, -1, false));
		assertEquals(4, HotbarScroll.move(3, 1, false));
		assertEquals(0, HotbarScroll.move(8, 1, true));
		assertEquals(8, HotbarScroll.move(0, -1, true));
		assertEquals(0, HotbarScroll.move(0, -10, false));
		assertEquals(8, HotbarScroll.move(8, 10, false));
	}

	@Test
	void freshConfigWrapsAndAnOlderFileStillWraps(@TempDir java.nio.file.Path dir) throws Exception {
		assertTrue(new ClientConfig().hotbarScrollWrap);
		Files.write(dir.resolve("arctic.json"), "{}".getBytes(StandardCharsets.UTF_8));
		assertTrue(ClientConfig.load(dir.toFile()).hotbarScrollWrap);
	}

	@Test
	void anExplicitOffStaysOff(@TempDir java.nio.file.Path dir) throws Exception {
		Files.write(dir.resolve("arctic.json"), "{\"hotbarScrollWrap\":false}".getBytes(StandardCharsets.UTF_8));
		assertFalse(ClientConfig.load(dir.toFile()).hotbarScrollWrap);
		Files.write(dir.resolve("arctic.json"), "{\"hotbarScrollWrap\":true}".getBytes(StandardCharsets.UTF_8));
		assertTrue(ClientConfig.load(dir.toFile()).hotbarScrollWrap);
	}
}
