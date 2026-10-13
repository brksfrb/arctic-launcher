package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class RecorderTest {
	@TempDir Path dir;
	final AtomicLong clock = new AtomicLong();
	private Recorder recorder(long maxBytes, long freeBytes) {
		return new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, key) -> {
			key[0] = 1;
			return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, maxBytes, freeBytes, 10_000, clock::get);
	}
	private Recorder recorder() { return recorder(512L << 20, 0); }
	private void at(int millis) { clock.set(millis * 1_000_000L); }
	private static void await(Recorder r) throws Exception {
		CountDownLatch done = new CountDownLatch(1); r.afterWrites(done::countDown);
		assertTrue(done.await(10, TimeUnit.SECONDS), "writer stalled");
	}
	private File clip(Recorder r) throws Exception {
		AtomicReference<File> saved = new AtomicReference<>();
		assertTrue(r.saveMoment(saved::set)); await(r);
		assertNotNull(saved.get()); return saved.get();
	}
	private static SelfSample sample(double x) {
		SelfSample s = new SelfSample(); s.x = x; s.y = 64; s.slot = 3; s.health = 20; s.fov = 70; return s;
	}
	@Test void instantClipWorksWithoutManualRecordingAndRetainsBootstrap() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("s", false, 775, "26.3");
		UUID uuid = UUID.randomUUID(); t.self(42, "Steve", uuid);
		t.packet(new byte[] {1, 42}); t.sample(sample(0));
		at(20_000); t.packet(new byte[] {2, 20}); t.sample(sample(20));
		at(40_000); t.packet(new byte[] {2, 40}); t.sample(sample(40));
		at(60_000); t.packet(new byte[] {2, 60}); t.sample(sample(60));
		assertFalse(r.recording());
		try (ReplayData d = ReplayData.open(clip(r), dir.resolve("cache").toFile())) {
			assertEquals(30_000, d.duration()); assertEquals(3, d.count());
			assertEquals(42, d.packet(0).get(1)); assertEquals(0, d.time(0));
			assertEquals(40, d.packet(1).get(1)); assertEquals(10_000, d.time(1));
			assertEquals(60, d.packet(2).get(1)); assertEquals(30_000, d.time(2));
			assertEquals(uuid.toString(), d.extras.selfUuid); assertEquals(42, d.meta.selfId);
			assertTrue(d.self.length <= 3); assertEquals(0, d.self[0].time);
		}
		t.end(); await(r); assertEquals(0, r.tempDir().listFiles().length);
	}
	@Test void clipsDoNotStopRewriteOrTruncateManualRecording() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("s", true, 775, "26.3");
		t.packet(new byte[] {1, 10});
		at(10_000); AtomicReference<File> manual = new AtomicReference<>();
		assertTrue(r.startRecording(manual::set));
		at(20_000); t.packet(new byte[] {2, 20});
		at(40_000); t.packet(new byte[] {2, 40}); File first = clip(r); byte[] bytes = Files.readAllBytes(first.toPath());
		assertTrue(r.recording()); assertNull(manual.get());
		at(70_000); t.packet(new byte[] {2, 70}); File second = clip(r);
		assertNotEquals(first, second); assertTrue(r.recording());
		at(80_000); t.packet(new byte[] {2, 80}); assertTrue(r.stopRecording()); await(r);
		assertFalse(r.recording()); assertNotNull(manual.get());
		assertArrayEquals(bytes, Files.readAllBytes(first.toPath()));
		try (ReplayData d = ReplayData.open(manual.get(), dir.resolve("manual").toFile())) {
			assertEquals(70_000, d.duration()); assertEquals(5, d.count());
			assertEquals(20, d.packet(1).get(1)); assertEquals(10_000, d.time(1));
			assertEquals(80, d.packet(4).get(1));
		}
		assertNotNull(clip(r)); t.end(); await(r);
	}
	@Test void startStopStartWhileWriterIsBusyAndLeavingSavesEachRecording() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("s", true, 1, "26.3"); t.packet(new byte[] {1});
		AtomicReference<File> first = new AtomicReference<>(), second = new AtomicReference<>();
		at(1_000); assertTrue(r.startRecording(first::set)); at(2_000); t.packet(new byte[] {2, 2});
		assertTrue(r.stopRecording()); at(3_000); assertTrue(r.startRecording(second::set));
		at(4_000); t.packet(new byte[] {2, 4}); t.end(); await(r);
		assertNotNull(first.get()); assertNotNull(second.get()); assertNotEquals(first.get(), second.get());
		try (ReplayData d = ReplayData.open(second.get(), dir.resolve("second").toFile())) {
			assertEquals(1_000, d.duration()); assertEquals(2, d.count()); assertEquals(4, d.packet(1).get(1));
		}
		assertNull(r.current()); assertFalse(r.recording()); assertEquals(0, r.tempDir().listFiles().length);
	}
	@Test void storageLimitUsesCompressedSizeAndRollingSelfTrackStaysSmall() throws Exception {
		Recorder r = recorder(100_000, 0); Recorder.Take t = r.begin("s", false, 1, "26.3");
		byte[] chunk = new byte[1_000_000]; chunk[0] = 1; t.packet(chunk);
		for (int i = 1; i <= 240; i++) { at(i * 1_000); t.packet(new byte[] {2}); t.sample(sample(i)); }
		await(r); assertTrue(r.startRecording(f -> {}), "compressed world data should fit"); assertTrue(r.stopRecording());
		try (ReplayData d = ReplayData.open(clip(r), dir.resolve("rolling").toFile())) {
			assertEquals(30_000, d.duration()); assertTrue(d.count() <= 32); assertTrue(d.self.length <= 32);
		}
		t.end(); await(r);
	}
	@Test void lowDiskStopsBufferWithoutDiscardingExistingData() throws Exception {
		Recorder r = recorder(1L << 30, Long.MAX_VALUE); Recorder.Take t = r.begin("s", false, 1, "26.3");
		t.packet(new byte[] {1}); await(r);
		assertFalse(r.startRecording(f -> {})); assertNotNull(clip(r)); t.end(); await(r);
	}
	@Test void aPausedWorldDoesNotLengthenTheManualRecording() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("s", true, 1, "26.3"); t.packet(new byte[] {1});
		AtomicReference<File> saved = new AtomicReference<>(); r.startRecording(saved::set);
		at(5_000); t.paused(true); at(60_000); assertEquals(5_000, t.time()); t.paused(false);
		at(65_000); t.packet(new byte[] {2}); r.stopRecording(); await(r);
		try (ReplayData d = ReplayData.open(saved.get(), dir.resolve("pause").toFile())) { assertEquals(10_000, d.duration()); }
		t.end(); await(r);
	}
	@Test void nothingIsSavedWithoutEitherControl() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("s", true, 1, "26.3"); t.packet(new byte[] {1}); t.end(); await(r);
		assertFalse(r.savedDir().exists()); assertFalse(r.saveMoment(null)); assertEquals(0, r.tempDir().listFiles().length);
	}
}
