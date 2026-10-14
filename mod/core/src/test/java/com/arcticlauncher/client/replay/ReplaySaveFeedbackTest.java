package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.notice.Notices;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplaySaveFeedbackTest {
	@TempDir Path dir;

	@Test void feedbackAppearsBeforeDeferredThumbnailAndCompletion() throws Exception {
		AtomicReference<ReplayBackend.FrameSink> capture = new AtomicReference<>();
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(),
				new Class<?>[] {ReplayBackend.class}, (proxy, method, args) -> {
					if (method.getName().equals("capture")) capture.set((ReplayBackend.FrameSink) args[0]);
					return null;
				});
		Recorder recorder = new Recorder(dir.toFile(), () -> 30, message -> {}, () -> (p, k) -> Category.ALWAYS,
				1L << 20, 0, 10_000);
		Field r = field(Replays.class, "recorder"), b = field(Replays.class, "backend"),
				e = field(Replays.class, "exports"), c = field(ArcticClient.class, "config");
		Object oldR = r.get(null), oldB = b.get(null), oldE = e.get(null), oldC = c.get(null);
		@SuppressWarnings("unchecked") List<Object> notices = (List<Object>) field(Notices.class, "QUEUE").get(null);
		List<Object> oldNotices = new ArrayList<>(notices);
		Recorder.Take take = recorder.begin("test", true, 1, "test"); take.packet(new byte[] {1});
		try {
			r.set(null, recorder); b.set(null, backend); e.set(null, new ExportQueue(dir.toFile())); c.set(null, new ClientConfig());
			notices.clear(); Replays.saveMoment();
			assertNotNull(capture.get());
			assertEquals("Saving clip", title(notices.get(notices.size() - 1)), "F9 must acknowledge before GPU readback or saving finishes");
			assertEquals(0, recorder.pendingSaves());
			capture.get().frame(1, 1, ByteBuffer.wrap(new byte[] {0, 0, 0, -1}), false);
			CountDownLatch done = new CountDownLatch(1); recorder.afterWrites(done::countDown);
			assertTrue(done.await(10, TimeUnit.SECONDS)); assertEquals(0, recorder.pendingSaves());
			assertEquals(1, Replays.queuedVideos());
			assertEquals("3D clip saved", title(notices.get(notices.size() - 1)));
			take.end(); Replays.saveMoment();
			assertEquals("Nothing to save yet", title(notices.get(notices.size() - 1)));
		} finally {
			take.end();
			CountDownLatch ended = new CountDownLatch(1); recorder.afterWrites(ended::countDown);
			assertTrue(ended.await(10, TimeUnit.SECONDS));
			r.set(null, oldR); b.set(null, oldB); e.set(null, oldE); c.set(null, oldC);
			notices.clear(); notices.addAll(oldNotices);
		}
	}

	private static String title(Object notice) throws Exception { return (String) field(notice.getClass(), "title").get(notice); }
	private static Field field(Class<?> type, String name) throws Exception {
		Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
	}
}
