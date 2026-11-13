package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.notice.Notices;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Recording asked for inside a world that was never buffered (Replay off at the join, or on since). */
class ReplayInWorldTest {
	@TempDir Path dir;

	private static Platform platform(boolean inWorld, Recorder recorder, AtomicInteger begun, boolean supported) {
		return (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (p, m, a) -> {
			switch (m.getName()) {
				case "inWorld": return inWorld;
				case "beginReplayInWorld":
					begun.incrementAndGet();
					if (supported) recorder.begin("world", true, 1, "test");
					return supported;
				case "keyLabel": return "key";
				case "atTitleScreen": return false;
				default: return m.getReturnType() == boolean.class ? (Object) false : null;
			}
		});
	}

	private void run(boolean inWorld, boolean supported, boolean enabled, boolean expectStarted, String expectedNotice) throws Exception {
		// These assertions use English prose, independent of which test initialized Lang first.
		Lang.init(null, "en");
		Recorder recorder = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, k) -> Category.ALWAYS, 1L << 20, 0, 10_000);
		AtomicInteger begun = new AtomicInteger();
		Field r = field(Replays.class, "recorder"), pl = field(Replays.class, "platform"), c = field(ArcticClient.class, "config");
		Object oldR = r.get(null), oldP = pl.get(null), oldC = c.get(null);
		@SuppressWarnings("unchecked") List<Object> notices = (List<Object>) field(Notices.class, "QUEUE").get(null);
		List<Object> oldNotices = new ArrayList<>(notices);
		try {
			ClientConfig config = new ClientConfig();
			config.replayEnabled = enabled;
			r.set(null, recorder); pl.set(null, platform(inWorld, recorder, begun, supported)); c.set(null, config);
			notices.clear();
			assertNull(recorder.current(), "nothing was buffering at the join");
			Replays.toggleRecording();
			assertEquals(expectStarted, Replays.recording());
			if (expectedNotice != null) assertEquals(expectedNotice, text(notices.get(notices.size() - 1)));
			if (expectStarted) {
				assertNotNull(recorder.current());
				assertEquals(1, begun.get());
				Replays.toggleRecording();
				assertFalse(Replays.recording());
				assertEquals(1, begun.get(), "stopping must not build another snapshot");
			}
		} finally {
			if (recorder.current() != null) recorder.current().end();
			// End queues filesystem cleanup and a save callback that posts to Notices.
			// Drain both before restoring shared state or letting @TempDir delete the files.
			CountDownLatch drained = new CountDownLatch(1);
			recorder.afterWrites(drained::countDown);
			assertTrue(drained.await(10, TimeUnit.SECONDS), "replay cleanup did not finish");
			r.set(null, oldR); pl.set(null, oldP); c.set(null, oldC);
			notices.clear(); notices.addAll(oldNotices);
		}
	}

	@Test void startsAnInWorldBufferWhenNoneExists() throws Exception { run(true, true, true, true, null); }
	@Test void saysRejoinWhenTheVersionCannotSnapshot() throws Exception {
		run(true, false, true, false, "This version can't record a world you were already in. Rejoin it, then record.");
	}
	@Test void saysReplayIsOffWhenDisabled() throws Exception {
		run(true, true, false, false, "Replay is off. Turn it on in the Replays tab.");
	}
	@Test void saysJoinWhenNotInAWorld() throws Exception { run(false, true, true, false, "Join a world first."); }

	private static String text(Object notice) throws Exception { return (String) field(notice.getClass(), "body").get(notice); }
	private static Field field(Class<?> type, String name) throws Exception {
		Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
	}
}
