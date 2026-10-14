package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LiveVideoTest {
	@TempDir Path dir;

	@Test void savesOverlappingInstantClipAndManualRecordingWithoutReplayingOrInterruptingCapture() throws Exception {
		Ffmpeg encoder = new Ffmpeg(0, null, true); File tool = encoder.get();
		assumeTrue(tool != null, "Native FFmpeg is required for the encoder integration test");
		ExecutorService saves = Executors.newSingleThreadExecutor();
		List<String> notices = Collections.synchronizedList(new ArrayList<String>());
		ReplayBackend backend = (ReplayBackend) Proxy.newProxyInstance(ReplayBackend.class.getClassLoader(), new Class<?>[]{ReplayBackend.class}, (p, method, args) -> {
			throw new AssertionError("Live capture must never call replay playback: " + method.getName());
		});
		File videos = dir.resolve("videos").toFile();
		LiveVideoSession session = new LiveVideoSession(dir.resolve("buffer").toFile(), videos, encoder, tool, "libx264", backend,
				640, 360, 30, saves, notices::add);
		boolean began = false, clipped = false, stopped = false;
		long start = System.nanoTime();
		try {
			while (System.nanoTime() - start < TimeUnit.MILLISECONDS.toNanos(4200)) {
				long ms = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start);
				ByteBuffer rgba = ByteBuffer.allocate(640 * 360 * 4);
				for (int i = 0; i < 640 * 360; i++) rgba.put((byte) (ms < 2000 ? 255 : 0)).put((byte) 0).put((byte) (ms < 2000 ? 0 : 255)).put((byte) 255);
				rgba.flip(); session.offer(640, 360, rgba, true);
				if (ms >= 300 && !began) { began = true; session.startRecording(); }
				if (ms >= 2400 && !clipped) { clipped = true; session.clip(1); }
				if (ms >= 3600 && !stopped) { stopped = true; session.stopRecording(); }
				assertNull(session.error(), notices.toString());
				Thread.sleep(20);
			}
		} finally { session.finish(); session.awaitFinish(10000); }
		saves.shutdown(); assertTrue(saves.awaitTermination(20, TimeUnit.SECONDS));
		assertNull(session.error(), notices.toString());
		assertEquals(0, LiveVideoSession.pending());
		File[] files = videos.listFiles((d, name) -> name.endsWith(".mp4") && !name.contains(".part."));
		assertNotNull(files); assertEquals(2, files.length, notices.toString());
		for (File file : files) {
			JsonObject probe = probe(file);
			JsonArray streams = probe.getAsJsonArray("streams");
			assertEquals(2, streams.size(), "Both H.264 video and AAC audio must be present");
			assertTrue(streams.toString().contains("video")); assertTrue(streams.toString().contains("audio"));
			JsonObject video = null, audio = null;
			for (JsonElement value : streams) {
				JsonObject stream = value.getAsJsonObject();
				if (stream.get("codec_type").getAsString().equals("video")) video = stream; else audio = stream;
			}
			assertNotNull(video); assertNotNull(audio);
			assertTrue(video.get("start_time").getAsDouble() < .04, "A clip must not begin with a missing partial GOP");
			assertEquals(video.get("duration").getAsDouble(), audio.get("duration").getAsDouble(), .08, "Audio and video timelines must stay aligned");
			double duration = probe.getAsJsonObject("format").get("duration").getAsDouble();
			if (file.getName().startsWith("Clip")) assertEquals(1, duration, .3);
			else assertEquals(3.3, duration, .4, "F9 must not reset or shorten the ongoing manual recording");
			assertTrue(file.length() < 1_000_000, "Encoded test footage should stay compressed");
		}
		assertFalse(notices.toString().contains("Couldn't"), notices.toString());
		assertFalse(notices.toString().contains("stopped:"), notices.toString());
	}

	private JsonObject probe(File file) throws Exception {
		Process p = new ProcessBuilder("ffprobe", "-v", "error", "-show_entries", "stream=codec_type,width,height,start_time,duration:format=duration", "-of", "json", file.getAbsolutePath()).start();
		ByteArrayOutputStream out = new ByteArrayOutputStream(); byte[] bytes = new byte[2048]; int n;
		while ((n = p.getInputStream().read(bytes)) >= 0) out.write(bytes, 0, n);
		assertTrue(p.waitFor(10, TimeUnit.SECONDS)); assertEquals(0, p.exitValue());
		return new JsonParser().parse(new String(out.toByteArray(), "UTF-8")).getAsJsonObject();
	}
}
