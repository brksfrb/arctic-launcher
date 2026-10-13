package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ExportQueueTest {
	@TempDir Path dir;
	@Test void queueSurvivesRestartDeduplicatesAndRejectsOutsideFiles() throws Exception {
		Path folder = Files.createDirectories(dir.resolve("replay_recordings"));
		File clip = Files.write(folder.resolve("clip.mcpr"), new byte[] {1}).toFile();
		ExportQueue q = new ExportQueue(dir.toFile()); q.add(clip); q.add(clip);
		q = new ExportQueue(dir.toFile()); assertEquals(1, q.size()); assertEquals(clip, q.next());
		ExportQueue loaded = q;
		assertThrows(java.io.IOException.class, () -> loaded.add(dir.resolve("outside.mcpr").toFile()));
		q.remove(clip); assertEquals(0, new ExportQueue(dir.toFile()).size()); assertTrue(clip.isFile());
	}
	@Test void videoDimensionsAreBoundedEvenAndPreservePortraitOrientation() {
		assertArrayEquals(new int[] {1280, 720}, VideoExport.outputSize(3840, 2160));
		assertArrayEquals(new int[] {404, 720}, VideoExport.outputSize(1080, 1920));
		assertArrayEquals(new int[] {854, 480}, VideoExport.outputSize(855, 481));
	}
}
