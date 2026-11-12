package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.Platform;
import java.io.DataOutputStream;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.function.IntPredicate;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplayViewerFailureTest {
	@TempDir Path dir;

	private final class Session implements AutoCloseable {
		int attempts, stops;
		final ReplayData data;
		boolean ready;
		final List<String> logs = new ArrayList<>();
		final ReplayViewer viewer;

		Session(boolean joined, boolean classified, IntPredicate fails, boolean terminal) throws Exception {
			ready = joined;
			Path file = dir.resolve("failure.mcpr");
			try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
				zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
				zip.write("{\"duration\":10000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
				zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY));
				DataOutputStream out = new DataOutputStream(zip);
				for (int i = 0; i < 100; i++) { out.writeInt(0); out.writeInt(1); out.writeByte(i); }
				zip.closeEntry();
			}
			ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[]{ReplayBackend.class}, (p, method, args) -> {
				switch (method.getName()) {
					case "ready": return ready;
					case "apply":
						if (fails.test(++attempts)) {
							if (terminal) throw new ReplayBackend.ProtocolFailure("phase=CONFIGURATION packetId=3", null);
							throw new IllegalStateException("phase=LOGIN packetId=51");
						}
						break;
					case "look": return new float[]{0, 0};
					case "stop": stops++; break;
				}
				return method.getReturnType() == boolean.class ? false : null;
			});
			Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class}, (p, method, args) -> {
				if (method.getName().equals("log")) logs.add((String) args[1]);
				return method.getReturnType() == boolean.class ? false : null;
			});
			data = ReplayData.open(file.toFile(), dir.resolve("cache").toFile());
			if (classified) data.classify((packet, key) -> Category.ALWAYS);
			viewer = new ReplayViewer(data, backend, platform, Collections.emptyList(), () -> {});
			assertNull(viewer.begin());
		}

		void assertStopped(int expectedAttempts, int expectedFailureLogs) {
			assertEquals(expectedAttempts, attempts);
			assertNotNull(viewer.failure());
			assertNotNull(viewer.takeProblem());
			assertNull(viewer.takeProblem());
			assertNull(viewer.status());
			assertTrue(viewer.paused());
			viewer.frame(); viewer.afterFrame(); viewer.seek(10000); viewer.frame();
			assertEquals(expectedAttempts, attempts, "Dismissing the problem or seeking must not resume packet feeding");
			assertEquals(expectedFailureLogs, logs.stream().filter(s -> s.contains("failed:")).count());
			assertTrue(logs.stream().anyMatch(s -> s.contains("phase=") && s.contains("packetId=")));
		}

		public void close() { viewer.end(); }
	}

	@Test void brokenJoinStopsInsideTheBatchAndLogsTheFirstFailureOnly() throws Exception {
		try (Session s = new Session(false, false, i -> true, false)) {
			s.viewer.frame(); s.assertStopped(ReplayViewer.MAX_PACKET_FAILURES, 1);
		}
	}

	@Test void normalPlaybackStopsFeedingAfterRepeatedFailures() throws Exception {
		try (Session s = new Session(true, false, i -> true, false)) {
			s.viewer.frame(); s.viewer.frame();
			s.assertStopped(ReplayViewer.MAX_PACKET_FAILURES, 1);
		}
	}

	@Test void unclassifiedSeekStopsAtTheFailureLimit() throws Exception {
		try (Session s = new Session(true, false, i -> true, false)) {
			s.viewer.frame(); s.viewer.seek(10000); s.viewer.frame();
			s.assertStopped(ReplayViewer.MAX_PACKET_FAILURES, 1);
		}
	}

	@Test void plannedSeekStopsAtTheFailureLimit() throws Exception {
		try (Session s = new Session(true, true, i -> true, false)) {
			s.viewer.frame(); s.viewer.seek(10000); s.viewer.frame();
			s.assertStopped(ReplayViewer.MAX_PACKET_FAILURES, 1);
		}
	}

	@Test void aSuccessfulPacketResetsTheConsecutiveFailureCount() throws Exception {
		try (Session s = new Session(false, false, i -> i != ReplayViewer.MAX_PACKET_FAILURES, false)) {
			s.viewer.frame(); s.assertStopped(ReplayViewer.MAX_PACKET_FAILURES * 2, 2);
		}
	}

	@Test void aBrokenJoinClosesAndReleasesTheReplayEvenWithoutAWorldOrHud() throws Exception {
		java.lang.reflect.Field globalViewer = Replays.class.getDeclaredField("viewer"); globalViewer.setAccessible(true);
		java.lang.reflect.Field inFrame = Replays.class.getDeclaredField("inFrame"); inFrame.setAccessible(true);
		Object previousViewer = globalViewer.get(null), previousInFrame = inFrame.get(null);
		try (Session s = new Session(false, false, i -> true, false)) {
			try {
				globalViewer.set(null, s.viewer); inFrame.set(null, false);
				Replays.frame(true);
				assertNull(Replays.viewer()); assertEquals(1, s.stops);
				assertEquals(ReplayViewer.MAX_PACKET_FAILURES, s.attempts);
				assertNotNull(s.viewer.failure());
				assertThrows(java.io.UncheckedIOException.class, () -> s.data.packet(0));
			} finally {
				globalViewer.set(null, previousViewer); inFrame.set(null, previousInFrame);
			}
		}
	}

	@Test void aFailedProtocolHandoverStopsImmediately() throws Exception {
		try (Session s = new Session(false, false, i -> true, true)) {
			s.viewer.frame(); s.assertStopped(1, 1);
		}
	}
}
