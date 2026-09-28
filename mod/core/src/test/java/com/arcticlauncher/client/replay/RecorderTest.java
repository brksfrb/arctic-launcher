package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class RecorderTest {
	@TempDir
	Path dir;

	private Recorder recorder(boolean on) {
		return new Recorder(dir.toFile(), () -> on, () -> 20, message -> {});
	}

	private static void await(Recorder r) throws InterruptedException {
		CountDownLatch done = new CountDownLatch(1);
		r.afterWrites(done::countDown);
		assertTrue(done.await(10, TimeUnit.SECONDS));
	}

	private static SelfSample sample(double x) {
		SelfSample s = new SelfSample();
		s.x = x;
		s.y = 64;
		s.yaw = 350;
		s.slot = 3;
		s.health = 20;
		s.fov = 70;
		return s;
	}

	@Test
	void aKeptSessionReadsBackWithItsPacketsSamplesAndMoments() throws Exception {
		Recorder r = recorder(true);
		Recorder.Take take = r.begin("play.example.net", false, 775, "26.3");
		UUID me = UUID.randomUUID();
		take.self(42, "Steve", me);
		take.packet(new byte[] {1, 2, 3});
		take.sample(sample(1));
		take.packet(new byte[] {4});
		take.sample(sample(2));
		AtomicReference<File> saved = new AtomicReference<File>();
		CountDownLatch kept = new CountDownLatch(1);
		assertTrue(r.saveMoment(f -> {
			saved.set(f);
			kept.countDown();
		}));
		assertTrue(kept.await(10, TimeUnit.SECONDS));
		assertNotNull(saved.get());
		assertTrue(saved.get().getName().startsWith("play.example.net "));

		try (ReplayData data = ReplayData.open(saved.get(), dir.resolve("cache").toFile())) {
			assertEquals(2, data.count());
			ByteBuffer first = data.packet(0);
			byte[] bytes = new byte[first.remaining()];
			first.get(bytes);
			assertArrayEquals(new byte[] {1, 2, 3}, bytes);
			assertEquals(1, data.packet(1).remaining());
			assertEquals("26.3", data.meta.mcversion);
			assertEquals(775, data.meta.protocol);
			assertEquals(42, data.meta.selfId);
			assertEquals("Steve", data.extras.selfName);
			assertEquals(me.toString(), data.extras.selfUuid);
			assertEquals(1, data.extras.moments.size());
			assertEquals(20, data.extras.clipSeconds);
			SelfSample s = data.selfAt(0);
			assertNotNull(s);
			assertEquals(3, s.slot);
			assertEquals(350, s.yaw, 1e-6);
		}
		take.end();
		await(r);
	}

	@Test
	void anUnkeptSessionLeavesNothingBehind() throws Exception {
		Recorder r = recorder(true);
		Recorder.Take take = r.begin("Singleplayer", true, 775, "26.3");
		take.packet(new byte[] {9, 9});
		take.end();
		await(r);
		File[] left = r.tempDir().listFiles();
		assertTrue(left == null || left.length == 0);
		assertFalse(new File(dir.toFile(), "replay_recordings").exists());
		assertNull(r.current());
	}

	@Test
	void recordingOffRecordsNothing() {
		Recorder r = recorder(false);
		assertNull(r.begin("x", false, 1, "26.3"));
		assertFalse(r.saveMoment(null));
	}

	@Test
	void pausedTimeDoesNotCount() throws Exception {
		Recorder r = recorder(true);
		Recorder.Take take = r.begin("Singleplayer", true, 775, "26.3");
		take.paused(true);
		int before = take.time();
		Thread.sleep(60);
		assertEquals(before, take.time());
		take.paused(false);
		take.end();
		await(r);
	}

	@Test
	void findsPacketsByTimeAndInterpolatesTheSelf() throws Exception {
		Recorder r = recorder(true);
		Recorder.Take take = r.begin("s", false, 1, "26.3");
		take.sample(sample(0));
		Thread.sleep(120);
		take.sample(sample(10));
		take.packet(new byte[] {1});
		CountDownLatch kept = new CountDownLatch(1);
		AtomicReference<File> saved = new AtomicReference<File>();
		r.saveMoment(f -> {
			saved.set(f);
			kept.countDown();
		});
		assertTrue(kept.await(10, TimeUnit.SECONDS));
		try (ReplayData data = ReplayData.open(saved.get(), dir.resolve("cache").toFile())) {
			assertEquals(0, data.indexAfter(-1));
			assertEquals(1, data.indexAfter(data.time(0)));
			int a = data.self[0].time;
			int b = data.self[1].time;
			SelfSample mid = data.selfAt((a + b) / 2.0);
			assertTrue(mid.x > 2 && mid.x < 8, "x " + mid.x);
		}
		take.end();
		await(r);
	}
}
