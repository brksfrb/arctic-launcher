package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import com.google.gson.JsonParser;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.junit.jupiter.api.Test;

/** Every emote the cosmetics server offers can be played by the client. */
class CatalogEmotesTest {
	private static final File EMOTES = new File("../../crates/arctic-cosmetics/assets/emotes");

	@Test
	void everyCatalogEmoteParses() throws IOException {
		File[] files = EMOTES.listFiles((dir, name) -> name.endsWith(".animation.json"));
		assertTrue(files != null && files.length > 0, "no emotes in " + EMOTES.getAbsolutePath());
		for (File f : files) {
			String text = new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8);
			try {
				Animation.parse(new JsonParser().parse(text));
			} catch (RuntimeException e) {
				fail(f.getName() + ": " + e.getMessage());
			}
		}
	}
}
