package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class FreezeWatchdogTest {
	@TempDir Path dir;

	@Test void aStalledClientThreadProducesOneFreezeFileAndLogLine() throws Exception {
		final java.util.List<String> lines = new java.util.concurrent.CopyOnWriteArrayList<String>();
		FreezeWatchdog.logTo(new java.util.function.Consumer<String>() {
			public void accept(String line) { lines.add(line); }
		});
		File out = dir.resolve("logs/arctic-freezes").toFile();
		FreezeWatchdog.start(Thread.currentThread(), out);
		FreezeWatchdog.note("test event", 7);
		CrashDiagnostics.frame();
		Thread.sleep(3300);
		CrashDiagnostics.frame();
		File[] files = null;
		for (int i = 0; i < 40 && (files == null || files.length == 0 || lines.isEmpty()); i++) {
			Thread.sleep(100);
			files = out.listFiles();
		}
		assertNotNull(files);
		assertEquals(1, files.length);
		String text = new String(Files.readAllBytes(files[0].toPath()), StandardCharsets.UTF_8);
		assertTrue(text.contains("game froze for 3."), text);
		assertTrue(text.contains("Thread.sleep"), text);
		assertTrue(text.contains("test event 7"), text);
		assertEquals(1, lines.size());
		assertTrue(lines.get(0).startsWith("Arctic: game froze for 3."), lines.get(0));
	}
}
