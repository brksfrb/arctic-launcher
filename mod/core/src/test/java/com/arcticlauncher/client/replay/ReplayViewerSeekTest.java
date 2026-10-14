package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.Platform;
import java.io.DataOutputStream;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplayViewerSeekTest {
	@TempDir Path dir;

	private final class World implements AutoCloseable {
		boolean ready, seeking, failRestart, nestedFrame;
		int resets, followed = -1;
		float yaw = 37, pitch = -12;
		final List<Integer> packets = new ArrayList<>();
		final List<String> events = new ArrayList<>();
		ReplayViewer viewer;

		World() throws Exception { this(false); }

		World(boolean retiredEntity) throws Exception {
			Path file = dir.resolve("seek.mcpr");
			try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
				zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
				zip.write("{\"duration\":10000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
				zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY));
				DataOutputStream out = new DataOutputStream(zip);
				int[] times = {0, 100, 5000, 10000};
				for (int i = 0; i < times.length; i++) { out.writeInt(times[i]); out.writeInt(1); out.writeByte(i + 1); }
				zip.closeEntry();
			}
			ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[]{ReplayBackend.class}, (p, method, args) -> {
				switch (method.getName()) {
					case "ready": return ready;
					case "beginSeek": seeking = true; events.add("begin"); break;
					case "endSeek": seeking = false; events.add("end"); break;
					case "restart":
						assertTrue(seeking); resets++; ready = false; packets.clear(); yaw = pitch = 0; followed = -1;
						if (nestedFrame) {
							int before = events.size(); viewer.frame(); viewer.afterFrame();
							assertEquals(before, events.size(), "A nested game frame must not touch a half-rebuilt world");
						}
						return failRestart ? "reset failed" : null;
					case "apply": packets.add((int) ((ByteBuffer) args[0]).get()); ready = true; break;
					case "look": return new float[]{yaw, pitch};
					case "camera":
						if (!Float.isNaN((Float) args[3])) { yaw = (Float) args[3]; pitch = (Float) args[4]; }
						if (seeking) events.add("camera"); break;
					case "showSelf": if (seeking) events.add("body"); break;
					case "follow": followed = (Integer) args[0]; if (seeking) events.add("follow"); break;
				}
				return method.getReturnType() == boolean.class ? false : null;
			});
			Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class}, (p, method, args) -> {
				if (method.getName().equals("camera")) return new double[]{10, 64, 10, yaw, pitch, 70};
				return method.getReturnType() == boolean.class ? false : null;
			});
			ReplayData data = ReplayData.open(file.toFile(), dir.resolve("cache").toFile());
			data.classify(new ReplayBackend.Classifier() {
				public byte classify(ByteBuffer packet, long[] key) {
					if (!retiredEntity) return Category.ALWAYS;
					key[0] = 7;
					int value = packet.get(0);
					return value == 2 ? Category.ENTITY_ADD : value == 3 ? Category.ENTITY_REMOVE : Category.ALWAYS;
				}
				public int[] removedEntities(ByteBuffer packet) { return new int[]{7}; }
			});
			viewer = new ReplayViewer(data, backend, platform, Collections.emptyList(), () -> {});
			assertNull(viewer.begin()); viewer.frame(); viewer.togglePause();
			viewer.seek(7000); viewer.frame(); events.clear();
		}

		public void close() { viewer.end(); }
	}

	@Test void rewindRestoresWorldAndCameraBeforeEndingTheSeekAndKeepsPauseAndEditor() throws Exception {
		try (World world = new World()) {
			world.viewer.screenOpen(true);
			world.viewer.seek(100); world.viewer.frame();
			assertEquals(java.util.Arrays.asList(1, 2), world.packets);
			assertEquals(1, world.resets); assertFalse(world.seeking);
			assertEquals(37, world.yaw); assertEquals(-12, world.pitch);
			assertEquals("begin", world.events.get(0));
			assertTrue(world.events.contains("body")); assertTrue(world.events.contains("camera"));
			assertEquals("end", world.events.get(world.events.size() - 1));
			assertTrue(world.viewer.screenOpen()); assertTrue(world.viewer.paused());
			assertEquals(100, world.viewer.time()); assertNull(world.viewer.status());
		}
	}

	@Test void rewindReattachesFollowCameraAndDoesNotAdvanceByTheTimeSpentSeeking() throws Exception {
		try (World world = new World()) {
			world.viewer.follow(42, "Player"); world.viewer.togglePause();
			world.viewer.seek(100); world.viewer.frame();
			assertEquals(42, world.followed); assertEquals(ReplayViewer.Mode.FOLLOW, world.viewer.mode());
			assertEquals(100, world.viewer.time()); assertFalse(world.viewer.paused());
			assertTrue(world.events.indexOf("follow") < world.events.indexOf("end"));
		}
	}

	@Test void failedRestartAlwaysReleasesTheSeekGuard() throws Exception {
		try (World world = new World()) {
			world.failRestart = true; world.viewer.seek(100); world.viewer.frame();
			assertFalse(world.seeking); assertTrue(world.events.contains("end"));
		}
	}

	@Test void rewindingAnOlderRecordingSkipsEntitiesThatNoLongerExistAtTheTarget() throws Exception {
		try (World world = new World(true)) {
			world.viewer.seek(10000); world.viewer.frame();
			world.viewer.seek(7000); world.viewer.frame();
			assertEquals(java.util.Arrays.asList(1, 3), world.packets);
			assertEquals(1, world.resets); assertTrue(world.viewer.ready());
		}
	}

	@Test void aForcedFrameInsideThePacketHandlerDoesNotAdvanceTheReplay() throws Exception {
		try (World world = new World()) {
			world.nestedFrame = true;
			world.viewer.seek(100); world.viewer.frame();
			assertEquals(java.util.Arrays.asList(1, 2), world.packets);
			assertTrue(world.viewer.ready()); assertEquals(100, world.viewer.time());
		}
	}
}
