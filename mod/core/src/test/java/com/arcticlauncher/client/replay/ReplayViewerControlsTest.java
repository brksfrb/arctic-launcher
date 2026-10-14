package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.Platform;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplayViewerControlsTest {
	@TempDir Path dir;
	private double[] camera = {10, 64, 10, 0, 0, 70};

	private ReplayViewer viewer() throws Exception {
		Path file = dir.resolve("controls.mcpr");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
			zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
			zip.write("{\"duration\":10000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
			zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY)); zip.closeEntry();
		}
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[]{ReplayBackend.class}, (proxy, method, args) -> {
			if (method.getName().equals("ready")) return true;
			if (method.getName().equals("look")) return new float[]{0, 0};
			if (method.getName().equals("camera")) {
				camera = new double[]{(Double)args[0], (Double)args[1], (Double)args[2], 0, 0, 70};
			}
			return method.getReturnType() == boolean.class ? false : null;
		});
		Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class}, (proxy, method, args) -> {
			if (method.getName().equals("camera")) return camera.clone();
			if (method.getName().equals("keyDown")) return args[0] == GameKey.FORWARD;
			return method.getReturnType() == boolean.class ? false : null;
		});
		ReplayViewer viewer = new ReplayViewer(ReplayData.open(file.toFile(), dir.resolve("cache").toFile()), backend, platform, Collections.emptyList(), () -> {});
		assertNull(viewer.begin()); viewer.frame(); viewer.togglePause();
		return viewer;
	}

	@Test void closingTheEditorReleasesTheCameraWithoutAGameTick() throws Exception {
		ReplayViewer viewer = viewer();
		Field active = field(Replays.class, "viewer"); Object old = active.get(null);
		try {
			active.set(null, viewer);
			Replays.frame(true);
			viewer.addKeyframe();
			double before = camera[2];
			field(ReplayViewer.class, "lastFrame").setLong(viewer, System.nanoTime() - 100_000_000L);
			Replays.frame(false); // There was deliberately no Replays.tick after closing the menu.
			assertFalse(viewer.screenOpen());
			assertTrue(viewer.paused()); assertEquals(0, viewer.time());
			assertTrue(camera[2] > before, "A paused replay still lets the user fly to the next keyframe");
		} finally { active.set(null, old); viewer.end(); }
	}

	@Test void repeatedPausedKeyframesUpdateThePointAndPathEditsTakeEffect() throws Exception {
		ReplayViewer viewer = viewer();
		try {
			viewer.addKeyframe(); camera[0] = 20; viewer.addKeyframe();
			assertEquals(1, viewer.keyframes().size()); assertFalse(viewer.canPlayPath());
			assertEquals(20, viewer.keyframes().get(0).x);
			viewer.seek(5000); viewer.addKeyframe();
			assertEquals(1, viewer.keyframes().size(), "A pending jump must not save the old position at the wrong time");
			viewer.screenOpen(true); viewer.frame(); viewer.addKeyframe();
			assertTrue(viewer.canPlayPath());
			viewer.playPath(); assertEquals(ReplayViewer.Mode.PATH, viewer.mode());
			viewer.removeKeyframe(); assertEquals(ReplayViewer.Mode.FREE, viewer.mode());
			assertFalse(viewer.canPlayPath()); viewer.frame();
		} finally { viewer.end(); }
	}

	private static Field field(Class<?> type, String name) throws Exception {
		Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
	}
}
