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
	@Test void audioSurvivesCompactionAndManualRecordingKeepsItsOwnTimeline() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("audio", true, 775, "26.3");
		t.packet(new byte[] {1});
		AudioEvent music = new AudioEvent(); music.id = 1; music.resource = "minecraft:sounds/music/game.ogg";
		music.left = music.right = .5f; music.loop = true; t.audio(music);
		at(10_000); AtomicReference<File> manual = new AtomicReference<>(); assertTrue(r.startRecording(manual::set));
		at(20_000); t.audio(music.at(20_000, 0)); t.sample(sample(20));
		at(60_000); t.sample(sample(60)); File clip = clip(r);
		assertTrue(r.recording());
		at(70_000); assertTrue(r.stopRecording()); await(r);
		try (java.util.zip.ZipFile z = new java.util.zip.ZipFile(clip);
				java.io.DataInputStream in = new java.io.DataInputStream(z.getInputStream(z.getEntry(AudioEvent.ENTRY)))) {
			AudioEvent e = AudioEvent.read(in); assertEquals(0, e.time); assertEquals(30, e.position);
		}
		try (java.util.zip.ZipFile z = new java.util.zip.ZipFile(manual.get());
				java.io.DataInputStream in = new java.io.DataInputStream(z.getInputStream(z.getEntry(AudioEvent.ENTRY)))) {
			AudioEvent e = AudioEvent.read(in); assertEquals(0, e.time); assertEquals(10, e.position);
			e = AudioEvent.read(in); assertEquals(10_000, e.time); assertEquals(20, e.position);
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
	@Test void slowCompactionDoesNotBlockLivePacketsAndSnapshotsKeepBothSegments() throws Exception {
		CountDownLatch compacting = new CountDownLatch(1), resume = new CountDownLatch(1);
		AtomicReference<String> error = new AtomicReference<>();
		Recorder r = new Recorder(dir.toFile(), () -> 30, error::set, () -> (p, key) -> {
			if (Thread.currentThread().getName().equals("arctic-replay-compact")) {
				compacting.countDown();
				try { if (!resume.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("test timed out"); }
				catch (InterruptedException e) { throw new IllegalStateException(e); }
			}
			return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, 512L << 20, 0, 10_000, clock::get);
		Recorder.Take t = r.begin("crowded", false, 775, "26.3");
		try {
			t.packet(new byte[] {1, 42}); at(40_000); t.packet(new byte[] {2, 40});
			assertTrue(compacting.await(10, TimeUnit.SECONDS));
			// More than the entire queue budget passes through while cleanup is deliberately stuck.
			byte[] traffic = new byte[1024]; traffic[0] = 2;
			for (int tick = 0; tick < 80; tick++) {
				at(40_001 + tick);
				for (int player = 0; player < 1000; player++) t.packet(traffic);
				CountDownLatch written = new CountDownLatch(1); r.afterPackets(written::countDown);
				assertTrue(written.await(3, TimeUnit.SECONDS), "cleanup blocked the recording writer");
			}
			AtomicReference<File> saved = new AtomicReference<>(); CountDownLatch done = new CountDownLatch(1);
			assertTrue(r.saveMoment(file -> { saved.set(file); done.countDown(); }));
			assertTrue(done.await(10, TimeUnit.SECONDS), "saving a clip waited for cleanup");
			assertNotNull(saved.get()); assertNull(error.get());
			try (ReplayData d = ReplayData.open(saved.get(), dir.resolve("busy").toFile())) {
				assertEquals(80_002, d.count()); assertEquals(42, d.packet(0).get(1));
				assertEquals(30_000, d.duration());
			}
		} finally { resume.countDown(); t.end(); await(r); }
		assertEquals(0, r.tempDir().listFiles().length);
	}
	@Test void stoppingDuringCleanupStillKeepsTheLastValidClip() throws Exception {
		CountDownLatch compacting = new CountDownLatch(1), resume = new CountDownLatch(1);
		Recorder r = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, key) -> {
			if (Thread.currentThread().getName().equals("arctic-replay-compact")) {
				compacting.countDown();
				try { if (!resume.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("test timed out"); }
				catch (InterruptedException e) { throw new IllegalStateException(e); }
			}
			return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, 512L << 20, 0, 10_000, clock::get);
		Recorder.Take t = r.begin("overflow", false, 775, "26.3");
		try {
			t.packet(new byte[] {1, 42}); at(40_000); t.packet(new byte[] {2, 40});
			assertTrue(compacting.await(10, TimeUnit.SECONDS));
			t.packet(new byte[(64 << 20) + 1]); // Deliberately exceed the bounded queue.
			assertFalse(r.bufferReady());
			resume.countDown(); await(r);
			try (ReplayData data = ReplayData.open(clip(r), dir.resolve("last-good").toFile())) {
				assertEquals(2, data.count()); assertEquals(42, data.packet(0).get(1)); assertEquals(40, data.packet(1).get(1));
			}
		} finally { resume.countDown(); t.end(); await(r); }
		assertEquals(0, r.tempDir().listFiles().length);
	}
	@Test void pendingSavesTrackQueuedClipsWithoutInterruptingManualRecording() throws Exception {
		CountDownLatch saving = new CountDownLatch(1), resume = new CountDownLatch(1);
		Recorder r = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, key) -> {
			if (Thread.currentThread().getName().equals("arctic-replay-save")) {
				saving.countDown();
				try { if (!resume.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("test timed out"); }
				catch (InterruptedException e) { throw new IllegalStateException(e); }
			}
			return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, 512L << 20, 0, Integer.MAX_VALUE, clock::get);
		Recorder.Take t = r.begin("busy-save", false, 775, "26.3");
		AtomicReference<File> first = new AtomicReference<>(), second = new AtomicReference<>(), manual = new AtomicReference<>();
		try {
			t.packet(new byte[] {1, 42}); assertTrue(r.startRecording(manual::set));
			at(40_000); t.packet(new byte[] {2, 40});
			assertTrue(r.saveMoment(first::set)); assertTrue(saving.await(10, TimeUnit.SECONDS));
			assertEquals(1, r.pendingSaves());
			at(41_000); t.packet(new byte[] {2, 41}); assertTrue(r.saveMoment(second::set));
			CountDownLatch written = new CountDownLatch(1); r.afterPackets(written::countDown);
			assertTrue(written.await(3, TimeUnit.SECONDS), "Saving blocked incoming packets");
			assertEquals(2, r.pendingSaves()); assertTrue(r.recording());
			resume.countDown(); await(r);
			assertEquals(0, r.pendingSaves()); assertNotNull(first.get()); assertNotNull(second.get());
			try (ReplayData a = ReplayData.open(first.get(), dir.resolve("first-queued").toFile());
					ReplayData b = ReplayData.open(second.get(), dir.resolve("second-queued").toFile())) {
				assertEquals(2, a.count()); assertEquals(40, a.packet(1).get(1));
				assertEquals(3, b.count()); assertEquals(41, b.packet(2).get(1));
				assertEquals(30_000, a.duration()); assertEquals(30_000, b.duration());
			}
			assertTrue(r.stopRecording()); await(r); assertNotNull(manual.get()); assertEquals(0, r.pendingSaves());
		} finally { resume.countDown(); t.end(); await(r); }
	}
	@Test void failedFileWriteClearsPendingStatus() throws Exception {
		Recorder r = recorder(); Recorder.Take t = r.begin("unwritable", true, 1, "test"); t.packet(new byte[] {1});
		Files.write(r.savedDir().toPath(), new byte[] {1}); // A file occupies the directory's path.
		CountDownLatch notified = new CountDownLatch(1); AtomicReference<File> result = new AtomicReference<>();
		assertTrue(r.saveMoment(file -> { result.set(file); notified.countDown(); }));
		await(r); assertEquals(0, notified.getCount()); assertNull(result.get()); assertEquals(0, r.pendingSaves());
		t.end(); await(r); assertEquals(0, r.tempDir().listFiles().length);
	}
	@Test void clipOnlyIndexesBootstrapAndKeepsEveryRecentPacket() throws Exception {
		java.util.concurrent.atomic.AtomicInteger classified = new java.util.concurrent.atomic.AtomicInteger();
		Recorder r = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, key) -> {
			classified.incrementAndGet(); key[0] = 1;
			return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, 512L << 20, 0, Integer.MAX_VALUE, clock::get);
		Recorder.Take t = r.begin("index-prefix", true, 1, "test");
		t.packet(new byte[] {1, 1}); at(5_000); t.packet(new byte[] {1, 2});
		at(10_000); t.packet(new byte[] {2, 10}); // Exactly at the clip boundary is recent, not bootstrap.
		at(20_000); t.packet(new byte[] {2, 20}); at(40_000); t.packet(new byte[] {2, 40});
		File saved = clip(r); assertEquals(2, classified.get(), "Recent packets should not need an index");
		try (ReplayData data = ReplayData.open(saved, dir.resolve("prefix-only").toFile())) {
			assertEquals(4, data.count()); assertEquals(2, data.packet(0).get(1));
			assertEquals(10, data.packet(1).get(1)); assertEquals(0, data.time(1));
			assertEquals(20, data.packet(2).get(1)); assertEquals(40, data.packet(3).get(1));
			assertEquals(30_000, data.duration());
		}
		t.end(); await(r);
	}
	@Test void oldEntityLifetimesAreCompactedButTheClipAndManualHistoryRemainIntact() throws Exception {
		Recorder r = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> new PacketSorter(
				new String[][] {{"login_finished"}, {}, {"add_entity", "rotate_head", "set_entity_motion", "remove_entities"}},
				false, false), 512L << 20, 0, 10_000, clock::get);
		Recorder.Take t = r.begin("lifecycle", false, 1, "test");
		t.packet(new byte[] {0}); // Login, then play ids.
		AtomicReference<File> manual = new AtomicReference<>(); assertTrue(r.startRecording(manual::set));
		t.packet(new byte[] {0, 7});
		for (int i = 0; i < 5000; i++) { t.packet(new byte[] {1, 7, (byte) i}); t.packet(new byte[] {2, 7, (byte) i}); }
		at(5_000); t.packet(new byte[] {3, 1, 7});
		at(40_000); t.packet(new byte[] {0, 8}); await(r); // Compact the old lifetime.
		at(45_000); t.packet(new byte[] {1, 8, 1}); t.packet(new byte[] {1, 8, 2});
		at(50_000); t.packet(new byte[] {3, 1, 8}); await(r);
		File saved = clip(r);
		try (ReplayData data = ReplayData.open(saved, dir.resolve("entities").toFile())) {
			assertEquals(6, data.count()); // Login + old removal + every packet inside the clip.
			assertEquals(30_000, data.duration());
			assertEquals(20_000, data.time(2)); assertEquals(25_000, data.time(3));
			assertEquals(1, data.packet(3).get(2)); assertEquals(2, data.packet(4).get(2));
			assertEquals(3, data.packet(5).get(0));
		}
		assertTrue(r.stopRecording()); await(r);
		try (ReplayData data = ReplayData.open(manual.get(), dir.resolve("entity-manual").toFile())) {
			assertEquals(10_007, data.count()); assertEquals(50_000, data.duration());
		}
		t.end(); await(r);
	}
}
