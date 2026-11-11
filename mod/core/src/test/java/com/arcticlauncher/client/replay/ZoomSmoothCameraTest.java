package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Features;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.nio.file.Files;
import java.util.Collections;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ZoomSmoothCameraTest {
	@TempDir Path dir;
	private boolean smooth;
	private boolean inWorld = true;
	private boolean featureSupport = true;
	private int writes;
	private final ClientConfig config = new ClientConfig();
	private final Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(),
			new Class<?>[]{Platform.class}, (proxy, method, args) -> {
		String name = method.getName();
		if (name.equals("smoothCamera")) {
			if (args == null) return smooth;
			smooth = (Boolean) args[0]; writes++; return null;
		}
		if (name.equals("configDir")) return dir.resolve("config").toFile();
		if (name.equals("inWorld")) return inWorld;
		if (name.equals("hasFeatures")) return featureSupport;
		if (method.getReturnType() == boolean.class) return false;
		if (method.getReturnType() == int.class) return 0;
		if (method.getReturnType() == long.class) return 0L;
		if (method.getReturnType() == float.class) return 0f;
		if (method.getReturnType() == double.class) return 0d;
		return null;
	});
	private final Features features = new Features(platform, config);

	private void start() { features.simulate(true, false); features.tick(false); assertTrue(smooth); }
	private void release() { features.simulate(false, false); features.tick(false); }

	@Test void defaultIsSmoothAndReleaseRestores() {
		assertTrue(config.zoomSmoothCamera);
		start(); release(); assertFalse(smooth);
		int restored = writes; release(); features.stopZoom(); assertEquals(restored, writes);
	}
	@Test void preservesVanillaCinematic() { smooth = true; start(); release(); assertTrue(smooth); }
	@Test void guiRestores() { start(); features.tick(true); assertFalse(smooth); }
	@Test void disablingZoomRestores() { start(); config.zoomEnabled = false; features.tick(false); assertFalse(smooth); }
	@Test void disablingSmoothMidZoomRestores() {
		start(); config.zoomSmoothCamera = false; features.tick(false); assertFalse(smooth);
		release(); assertFalse(smooth);
	}
	@Test void disablingSmoothAndReleasingInTheSameTickRestores() {
		start(); config.zoomSmoothCamera = false; release(); assertFalse(smooth);
	}
	@Test void losingFeatureSupportRestores() {
		start();
		// A platform can lose its player while its world is still present.
		featureSupport = false; features.tick(false); assertFalse(smooth);
	}
	@Test void stoppingWithoutApplyingDoesNotTouchVanilla() {
		smooth = true; features.stopZoom(); features.stopZoom(); assertTrue(smooth); assertEquals(0, writes);
	}
	@Test void startingWithoutSmoothDoesNotTouchVanilla() {
		config.zoomSmoothCamera = false;
		for (boolean before : new boolean[]{false, true}) {
			smooth = before; features.simulate(true, false); features.tick(false); release();
			assertEquals(before, smooth);
		}
		assertEquals(0, writes);
	}
	@Test void worldLeaveAndDimensionGapRestoreThroughClientTick() throws Exception {
		withClient(() -> {
			start(); inWorld = false; ArcticClient.tick(false); assertFalse(smooth);
			ArcticClient.tick(false); assertFalse(smooth);
			inWorld = true; features.tick(false); assertTrue(smooth);
			release(); assertFalse(smooth);
		});
	}
	@Test void replayEarlyReturnRestoresThroughClientTick() throws Exception {
		Path file = dir.resolve("test.mcpr");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
			zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY)); zip.write("{\"duration\":1000}".getBytes("UTF-8")); zip.closeEntry();
			zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY)); zip.closeEntry();
		}
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(),
				new Class<?>[]{ReplayBackend.class}, (p, m, a) -> m.getReturnType() == boolean.class ? false : null);
		ReplayData data = ReplayData.open(file.toFile(), dir.resolve("cache").toFile());
		ReplayViewer viewer = new ReplayViewer(data, backend, platform, Collections.emptyList(), () -> {});
		Field v = field(Replays.class, "viewer"); Object old = v.get(null);
		try {
			withClient(() -> {
				start(); v.set(null, viewer); assertTrue(Replays.watching());
				ArcticClient.tick(false); assertFalse(smooth);
				ArcticClient.tick(false); assertFalse(smooth);
			});
		} finally { v.set(null, old); data.close(); }
	}
	private interface Check { void run() throws Exception; }
	private void withClient(Check check) throws Exception {
		Field[] fields = {field(ArcticClient.class, "platform"), field(ArcticClient.class, "config"),
			field(ArcticClient.class, "features"), field(Replays.class, "platform")};
		Object[] old = new Object[fields.length]; Object[] values = {platform, config, features, platform};
		for (int i = 0; i < fields.length; i++) { old[i] = fields[i].get(null); fields[i].set(null, values[i]); }
		try { check.run(); } finally { for (int i = 0; i < fields.length; i++) fields[i].set(null, old[i]); }
	}
	private static Field field(Class<?> type, String name) throws Exception {
		Field f = type.getDeclaredField(name); f.setAccessible(true); return f;
	}
}
