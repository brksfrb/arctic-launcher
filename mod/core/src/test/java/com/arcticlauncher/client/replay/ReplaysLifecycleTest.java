package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.nio.file.Files;
import java.nio.file.StandardOpenOption;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplaysLifecycleTest {
	@TempDir Path dir;
	@Test void replayDefaultsOffAndKeepsItsExistingSettingKey() throws Exception {
		assertFalse(ClientConfig.load(dir.toFile()).replayEnabled);
		Path file = dir.resolve("arctic.json");
		Files.write(file, "{\"replayEnabled\":true}".getBytes(StandardCharsets.UTF_8));
		ClientConfig config = ClientConfig.load(dir.toFile());
		assertTrue(config.replayEnabled);
		config.replayEnabled = false;
		assertNull(config.save());
		assertFalse(new com.google.gson.JsonParser().parse(new String(Files.readAllBytes(file), StandardCharsets.UTF_8))
				.getAsJsonObject().get("replayEnabled").getAsBoolean());
	}

	@Test void offStateCanBePromotedInPlaceAndOnlyTheEnabledWindowIsSaved() throws Exception {
		java.util.concurrent.atomic.AtomicLong clock = new java.util.concurrent.atomic.AtomicLong();
		Recorder recorder = new Recorder(dir.toFile(), () -> 30, ignored -> {}, () -> (packet, key) ->
				packet.get(0) == 2 ? Category.TRANSIENT : Category.ALWAYS,
				1L << 20, 0, 10_000, clock::get);
		Field config = field(ArcticClient.class, "config");
		Object old = config.get(null);
		Recorder.Take take = null;
		try {
			config.set(null, new ClientConfig());
			take = recorder.begin("test", true, 340, "1.12.2");
			assertEquals(Recorder.TakeState.STATE_ONLY, recorder.state());
			take.packet(new byte[] {1});
			clock.set(1_000_000_000L);
			take.packet(new byte[] {2});
			assertFalse(recorder.saveMoment(null));
			assertFalse(recorder.startRecording(null));

			clock.set(2_000_000_000L);
			recorder.enabled(true);
			assertSame(take, recorder.current());
			assertEquals(Recorder.TakeState.BUFFERING, recorder.state());
			take.packet(new byte[] {3});
			clock.set(3_000_000_000L);
			AtomicReference<File> saved = new AtomicReference<>();
			assertTrue(recorder.saveMoment(saved::set));
			CountDownLatch done = new CountDownLatch(1);
			recorder.afterWrites(done::countDown);
			assertTrue(done.await(10, TimeUnit.SECONDS));
			assertNotNull(saved.get());
			try (ReplayData data = ReplayData.open(saved.get(), dir.resolve("cache").toFile())) {
				assertEquals(1000, data.duration());
				assertEquals(2, data.count());
				assertEquals(1, data.packet(0).get());
				assertEquals(3, data.packet(1).get());
			}
		} finally {
			config.set(null, old);
			if (take != null) take.end();
			CountDownLatch done = new CountDownLatch(1);
			recorder.afterWrites(done::countDown);
			assertTrue(done.await(10, TimeUnit.SECONDS));
		}
		assertEquals(Recorder.TakeState.OFF, recorder.state());
		assertEquals(0, recorder.tempDir().listFiles().length);
	}

	@Test void reachingTitleFinalizesRecordingEvenWithoutConnectionCallback() throws Exception {
		AtomicBoolean atTitle = new AtomicBoolean();
		AtomicBoolean inWorld = new AtomicBoolean();
		Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
				(proxy, method, args) -> method.getName().equals("atTitleScreen") ? atTitle.get()
						: method.getName().equals("inWorld") ? inWorld.get() : false);
		ClientConfig config = new ClientConfig(); config.replayAutoExport = false; config.replayEnabled = true;
		Recorder recorder = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, k) -> Category.ALWAYS, 1L << 20, 0, 10_000);
		Field p = field(Replays.class, "platform"), r = field(Replays.class, "recorder"), c = field(ArcticClient.class, "config"), w = field(Replays.class, "worldTake");
		Object oldP = p.get(null), oldR = r.get(null), oldC = c.get(null), oldW = w.get(null);
		try {
			p.set(null, platform); r.set(null, recorder); c.set(null, config); w.set(null, null);
			Recorder.Take take = recorder.begin("test", true, 1, "test"); take.packet(new byte[] {1});
			atTitle.set(true); Replays.tick(true);
			assertSame(take, recorder.current(), "The title is still visible while older games log in");
			atTitle.set(false); inWorld.set(true); Replays.tick(false);
			AtomicReference<File> saved = new AtomicReference<>(); assertTrue(recorder.startRecording(saved::set));
			Replays.tick(true); assertTrue(recorder.recording(), "Other menus must not end a live recording");
			inWorld.set(false); atTitle.set(true); Replays.tick(true);
			assertNull(recorder.current()); assertFalse(recorder.recording());
			CountDownLatch done = new CountDownLatch(1); recorder.afterWrites(done::countDown);
			assertTrue(done.await(10, TimeUnit.SECONDS)); assertNotNull(saved.get()); assertTrue(saved.get().isFile());
		} finally {
			p.set(null, oldP); r.set(null, oldR); c.set(null, oldC); w.set(null, oldW);
		}
	}

	@Test void interruptedTakeRecoversOnlyThePublishedCheckpointAndDoesNotDuplicateIt() throws Exception {
		AtomicLong clock = new AtomicLong();
		Path live = dir.resolve("live");
		Path crashed = dir.resolve("crashed");
		Recorder recorder = new Recorder(live.toFile(), () -> 30, ignored -> {}, () -> (p, k) -> Category.ALWAYS,
				1L << 20, 0, 10_000, clock::get);
		Recorder.Take take = recorder.begin("test", true, 47, "1.8.9");
		recorder.enabled(true);
		try {
			take.packet(new byte[] {1});
			clock.set(TimeUnit.SECONDS.toNanos(1));
			assertTrue(recorder.startRecording(null));
			clock.set(TimeUnit.SECONDS.toNanos(2));
			assertTrue(recorder.mark());
			take.packet(new byte[] {2});
			waitForWriter(recorder);
			Path recovery = live.resolve(".arctic/replay-recovery");
			Path source;
			try (java.util.stream.Stream<Path> files = Files.list(recovery)) {
				source = files.findFirst().orElseThrow(AssertionError::new);
			}
			Path taken = crashed.resolve(".arctic/replay-recovery").resolve(source.getFileName());
			Files.createDirectories(taken);
			try (java.nio.file.DirectoryStream<Path> files = Files.newDirectoryStream(source)) {
				for (Path file : files) Files.copy(file, taken.resolve(file.getFileName()));
			}
			Files.write(taken.resolve("checkpoint.next"), new byte[] {'{'});
			Files.write(taken.resolve("take.tmcpr"), new byte[] {1, 2, 3}, StandardOpenOption.APPEND);

			waitForWriter(new Recorder(crashed.toFile(), () -> 30, ignored -> {}, () -> null));
			File[] files = crashed.resolve("replay_recordings").toFile().listFiles();
			assertNotNull(files);
			assertEquals(1, files.length);
			try (ReplayData data = ReplayData.open(files[0], crashed.resolve("cache").toFile())) {
				assertEquals(1000, data.duration());
				assertEquals(2, data.count());
				assertEquals(1, data.packet(0).get());
				assertEquals(2, data.packet(1).get());
				assertEquals(java.util.Collections.singletonList(1000), data.extras.markers);
			}
			assertFalse(Files.exists(taken));
			waitForWriter(new Recorder(crashed.toFile(), () -> 30, ignored -> {}, () -> null));
			assertEquals(1, crashed.resolve("replay_recordings").toFile().list().length);
		} finally {
			take.end();
			waitForWriter(recorder);
		}
	}

	private static void waitForWriter(Recorder recorder) throws InterruptedException {
		CountDownLatch done = new CountDownLatch(1);
		recorder.afterWrites(done::countDown);
		assertTrue(done.await(10, TimeUnit.SECONDS));
	}
	private static Field field(Class<?> type, String name) throws Exception {
		Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
	}
}
