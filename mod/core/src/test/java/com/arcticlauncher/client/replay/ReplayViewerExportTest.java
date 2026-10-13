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
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[] {ReplayBackend.class},
				(proxy, method, args) -> {
					if (method.getName().equals("ready")) return true;
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
				viewer.frame(); viewer.afterFrame();
				assertEquals(1, captures.get()); assertEquals(1000.0 / 30, job.frameTime());
			} finally { viewer.end(); }
		}
	}
}
