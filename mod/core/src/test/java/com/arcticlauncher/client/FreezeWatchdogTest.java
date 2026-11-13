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

	@Test void aStalledClientLeavesEvidenceBeforeRecoveryAndRewritesTheSameFile() throws Exception {
		final java.util.List<String> lines = new java.util.concurrent.CopyOnWriteArrayList<String>();
		FreezeWatchdog.logTo(new java.util.function.Consumer<String>() {
			public void accept(String line) { lines.add(line); }
		});
		File out = dir.resolve("logs/arctic-freezes").toFile();
		FreezeWatchdog.start(Thread.currentThread(), out);
		FreezeWatchdog.note("test event", 7);
		CrashDiagnostics.frame();
		// Windows may kill a non-responding window before the old 10 s first write.
		Path snapshot = awaitSnapshot(out, 3.0, 5000);
		String first = read(snapshot);
		assertTrue(first.contains("still frozen when written"), first);
		assertTrue(first.contains("Thread.sleep"), first);
		assertTrue(first.contains("test event 7"), first);
		assertTrue(first.contains("thread \""), first); // Other threads captured before the first write.
		assertTrue(lines.isEmpty(), lines.toString());
		assertEquals(snapshot, awaitSnapshot(out, 6.0, 4500));
		assertTrue(read(snapshot).contains("still frozen when written"));
		assertEquals(1, out.listFiles((d, n) -> n.endsWith(".txt")).length);
		assertTrue(lines.isEmpty(), lines.toString());
		CrashDiagnostics.frame();
		File[] files = null;
		for (int i = 0; i < 40 && (files == null || files.length == 0 || lines.isEmpty()); i++) {
			Thread.sleep(100);
			files = out.listFiles();
		}
		assertNotNull(files);
		assertEquals(1, files.length);
		String text = new String(Files.readAllBytes(files[0].toPath()), StandardCharsets.UTF_8);
		assertTrue(text.contains("game froze for "), text);
		assertFalse(text.contains("still frozen when written"), text);
		assertEquals(snapshot, files[0].toPath());
		assertTrue(text.contains("Thread.sleep"), text);
		assertTrue(text.contains("test event 7"), text);
		assertEquals(1, lines.size());
		assertTrue(lines.get(0).startsWith("Arctic: game froze for "), lines.get(0));
	}

	private static String read(Path path) throws Exception {
		return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
	}

	private static Path awaitSnapshot(File out, double seconds, long timeoutMs) throws Exception {
		long deadline = System.nanoTime() + timeoutMs * 1_000_000L;
		while (System.nanoTime() < deadline) {
			File[] files = out.listFiles((d, n) -> n.endsWith(".txt"));
			if (files != null && files.length == 1) {
				for (String line : read(files[0].toPath()).split("\n")) {
					if (line.startsWith("duration: ")) {
						double duration = Double.parseDouble(line.substring(10, line.indexOf('s', 10)));
						if (duration >= seconds) return files[0].toPath();
					}
				}
			}
			Thread.sleep(50);
		}
		throw new AssertionError("No in-progress freeze snapshot after " + seconds + " s");
	}
}
