package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.io.DataOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ReplayQueueSafetyTest {
	@TempDir Path dir;

	private final class Session implements AutoCloseable {
		final Map<Field, Object> previous = new LinkedHashMap<>();
		final LinkedBlockingQueue<Runnable> callbacks = new LinkedBlockingQueue<>();
		final ClientConfig config = new ClientConfig();
		final Path clip;
		boolean title = true, world;
		int starts;

		Session() throws Exception {
			for (Field f : Replays.class.getDeclaredFields()) {
				if (Modifier.isStatic(f.getModifiers()) && !Modifier.isFinal(f.getModifiers())) {
					f.setAccessible(true); previous.put(f, f.get(null));
					f.set(null, f.getType() == boolean.class ? false : f.getType() == int.class ? 0 : null);
				}
			}
			Field c = ArcticClient.class.getDeclaredField("config"); c.setAccessible(true);
			previous.put(c, c.get(null)); c.set(null, config);
			clip = dir.resolve("replay_recordings/clip.mcpr"); Files.createDirectories(clip.getParent());
			try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(clip))) {
				zip.putNextEntry(new ZipEntry(Recorder.META_ENTRY));
				zip.write("{\"duration\":1000}".getBytes(StandardCharsets.UTF_8)); zip.closeEntry();
				zip.putNextEntry(new ZipEntry(Recorder.PACKETS_ENTRY));
				DataOutputStream out = new DataOutputStream(zip);
				out.writeInt(0); out.writeInt(1); out.writeByte(1); zip.closeEntry();
			}
			new ExportQueue(dir.toFile()).add(clip.toFile());
			Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class}, (p, m, a) -> {
				switch (m.getName()) {
					case "configDir": return dir.resolve("config").toFile();
					case "atTitleScreen": return title;
					case "inWorld": return world;
					case "runOnGameThread": callbacks.add((Runnable) a[0]); return null;
				}
				return m.getReturnType() == boolean.class ? false : null;
			});
			Replays.init(platform, 0, null);
			Replays.backend((ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[]{ReplayBackend.class}, (p, m, a) -> {
				if (m.getName().equals("start")) starts++;
				return m.getReturnType() == boolean.class ? false : m.getReturnType() == int.class ? 0 : null;
			}));
		}

		Runnable unpackQueuedReplay() throws Exception {
			Replays.watch(clip.toFile());
			Runnable callback = callbacks.poll(10, TimeUnit.SECONDS); assertNotNull(callback); return callback;
		}

		public void close() throws Exception {
			Replays.liveVideo().close();
			Replays.stopWatching();
			if (Replays.recorder().current() != null) Replays.recorder().current().end();
			for (Map.Entry<Field, Object> e : previous.entrySet()) e.getKey().set(null, e.getValue());
		}
	}

	@Test void oldAutomaticSettingAndQueueCanNeverOpenAReplay() throws Exception {
		try (Session s = new Session()) {
			assertFalse(s.config.replayAutoExport, "Rendering in the game window must be opt-in");
			s.config.replayAutoExport = true;
			for (int i = 0; i < 300; i++) Replays.tick(true);
			assertEquals(1, Replays.legacyQueuedVideos()); assertEquals(0, Replays.queuedVideos());
			assertEquals(0, s.starts); assertTrue(s.callbacks.isEmpty());
		}
	}

	@Test void oldQueueCannotInterruptANewLiveConnection() throws Exception {
		try (Session s = new Session()) {
			s.config.replayAutoExport = true;
			s.title = false; s.world = true;
			Recorder.Take live = Replays.recorder().begin("test", true, 1, "test");
			for (int i = 0; i < 300; i++) Replays.tick(true);
			assertEquals(0, s.starts); assertNull(Replays.viewer());
			assertSame(live, Replays.recorder().current());
			assertEquals(1, Replays.legacyQueuedVideos()); assertTrue(Files.isRegularFile(s.clip));
		}
	}

	@Test void leavingThenReturningToTitleStillCancelsThePendingQueueOpen() throws Exception {
		try (Session s = new Session()) {
			s.config.replayAutoExport = true;
			s.title = false; Replays.tick(true);
			s.title = true; for (int i = 0; i < 300; i++) Replays.tick(true);
			assertEquals(0, s.starts); assertNull(Replays.viewer()); assertTrue(s.callbacks.isEmpty());
		}
	}

	@Test void explicitRenderWorksWithoutEnablingAutomaticRendering() throws Exception {
		try (Session s = new Session()) {
			s.unpackQueuedReplay().run();
			assertEquals(1, s.starts); assertNotNull(Replays.viewer()); assertFalse(s.config.replayAutoExport);
		}
	}
}
