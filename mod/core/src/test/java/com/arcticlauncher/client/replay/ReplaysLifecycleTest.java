package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplaysLifecycleTest {
	@TempDir Path dir;
	@Test void reachingTitleFinalizesRecordingEvenWithoutConnectionCallback() throws Exception {
		AtomicBoolean atTitle = new AtomicBoolean();
		AtomicBoolean inWorld = new AtomicBoolean();
		Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
				(proxy, method, args) -> method.getName().equals("atTitleScreen") ? atTitle.get()
						: method.getName().equals("inWorld") ? inWorld.get() : false);
		ClientConfig config = new ClientConfig(); config.replayAutoExport = false;
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
	private static Field field(Class<?> type, String name) throws Exception {
		Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
	}
}
