package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.Platform;
import java.io.DataOutputStream;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplayViewerExportTest {
	@TempDir Path dir;

	@Test void exportStartedDuringATickWaitsForTheRecordedCameraBeforeFrameZero() throws Exception {
		Path file = dir.resolve("clip.mcpr");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
			zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
			zip.write("{\"duration\":1000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
			zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY)); zip.closeEntry();
			zip.putNextEntry(new ZipEntry(Recorder.SELF_ENTRY));
			SelfSample sample = new SelfSample(); sample.x = 12; sample.y = 64; sample.fov = 70;
			sample.write(new DataOutputStream(zip)); zip.closeEntry();
		}
		AtomicInteger captures = new AtomicInteger();
		AtomicBoolean cameraPlaced = new AtomicBoolean();
		AtomicBoolean loading = new AtomicBoolean();
		AtomicBoolean terrainWaitRequested = new AtomicBoolean();
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[] {ReplayBackend.class},
				(proxy, method, args) -> {
					if (method.getName().equals("ready")) return true;
					if (method.getName().equals("loading")) return loading.get();
					if (method.getName().equals("beginSeek")) terrainWaitRequested.set((Boolean) args[0]);
					if (method.getName().equals("camera") && Boolean.TRUE.equals(args[6])) cameraPlaced.set(true);
					if (method.getName().equals("capture")) { assertTrue(cameraPlaced.get()); captures.incrementAndGet(); }
					return method.getReturnType() == boolean.class ? false : null;
				});
		Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (proxy, method, args) -> null);
		try (ReplayData data = ReplayData.open(file.toFile(), dir.resolve("cache").toFile())) {
			ReplayViewer viewer = new ReplayViewer(data, backend, platform, Collections.emptyList(), () -> {});
			try {
				assertNull(viewer.begin()); viewer.frame(); // Joined; this frame used the old camera.
				VideoExport job = new VideoExport(null, dir.resolve("clip.mp4").toFile(), 0, 1000, 30, null);
				viewer.export(job, ReplayViewer.Mode.FIRST_PERSON);
				viewer.afterFrame();
				assertEquals(0, captures.get()); assertEquals(0, job.frameTime());
				loading.set(true); viewer.frame(); viewer.afterFrame();
				assertTrue(terrainWaitRequested.get(), "Exports must keep waiting for the rebuilt terrain");
				assertEquals(0, captures.get(), "Loading terrain must not become the first video frame");
				assertEquals(0, job.frameTime(), "Waiting for terrain must not consume any recorded time");
				loading.set(false);
				viewer.frame(); viewer.afterFrame();
				assertEquals(1, captures.get()); assertEquals(1000.0 / 30, job.frameTime());
			} finally { viewer.end(); }
		}
	}

	/**
	 * A clip saved seconds after joining keeps its real timing: nothing before the login, then (a moment
	 * later) the position that ends the terrain screen. "Render" exports it from its first moment and asks
	 * for that before the world is up; the export must still get its first frame.
	 */
	@Test void renderingAClipFromItsFirstMomentLeavesTheTerrainScreen() throws Exception {
		Path file = dir.resolve("early.mcpr");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
			zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
			zip.write("{\"duration\":1000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
			zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY));
			DataOutputStream packets = new DataOutputStream(zip);
			packets.writeInt(70); packets.writeInt(1); packets.write(1);  // the login: the world exists from here
			packets.writeInt(110); packets.writeInt(1); packets.write(2); // the position, 40 ms later
			packets.writeInt(900); packets.writeInt(1); packets.write(3); // much later: not needed to load
			packets.flush(); zip.closeEntry();
		}
		AtomicInteger applied = new AtomicInteger();
		AtomicInteger restarts = new AtomicInteger();
		AtomicInteger captures = new AtomicInteger();
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[] {ReplayBackend.class},
				(proxy, method, args) -> {
					if (method.getName().equals("apply")) applied.incrementAndGet();
					// Like the game: a restart is an empty connection until the login is applied again.
					if (method.getName().equals("restart")) { restarts.incrementAndGet(); applied.set(0); }
					if (method.getName().equals("ready")) return applied.get() >= 1;
					// The terrain screen: up from the login until the position arrives.
					if (method.getName().equals("loading")) return applied.get() == 1;
					if (method.getName().equals("capture")) captures.incrementAndGet();
					if (method.getName().equals("look")) return new float[] {0, 0};
					return method.getReturnType() == boolean.class ? false : null;
				});
		Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (proxy, method, args) -> null);
		try (ReplayData data = ReplayData.open(file.toFile(), dir.resolve("cache-early").toFile())) {
			ReplayViewer viewer = new ReplayViewer(data, backend, platform, Collections.emptyList(), () -> {});
			try {
				assertNull(viewer.begin());
				VideoExport job = new VideoExport(null, dir.resolve("early.mp4").toFile(), 0, 1000, 30, null);
				viewer.export(job, ReplayViewer.Mode.FIRST_PERSON); // as the Replays tab does: with the open
				for (int i = 0; i < 120 && captures.get() == 0; i++) { viewer.frame(); viewer.afterFrame(); } // 30 a second
				assertEquals(0, restarts.get(), "nothing precedes the login to rebuild the world from");
				assertEquals(1, captures.get(), "the export stayed on the terrain screen");
				assertEquals(2, applied.get(), "waiting for terrain must not run ahead through the recording");
			} finally { viewer.end(); }
		}
	}
}
