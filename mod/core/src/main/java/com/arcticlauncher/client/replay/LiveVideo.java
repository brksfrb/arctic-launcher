package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.notice.Notices;
import java.io.File;
import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicBoolean;

/** Capture only the game's finished framebuffer. Encoding never owns the world or camera. */
public final class LiveVideo {
	private final Ffmpeg encoder;
	private final File root, videos;
	private volatile File ready;
	private volatile String codec = "libx264", state = "Preparing video encoder";
	private volatile String captureFailure;
	private volatile LiveVideoSession session;
	private volatile boolean closed;
	private Thread prepare;
	private Thread shutdown;
	private Recorder.Take lastTake;
	private boolean stoppedForTake;
	private long nextFrame;
	private long captureCost;
	private final AtomicBoolean capturing = new AtomicBoolean();
	private final java.util.Map<Integer, AudioEvent> sounds = new java.util.LinkedHashMap<Integer, AudioEvent>();
	private final java.util.concurrent.ExecutorService saves = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-video-save"); t.setDaemon(true); t.setPriority(Thread.MIN_PRIORITY); return t;
	});

	LiveVideo(File gameDir, int port, String secret) {
		root = new File(gameDir, ".arctic/live-video"); videos = new File(gameDir, "replay_videos");
		encoder = new Ffmpeg(port, secret, true);
		prepare = new Thread(() -> {
			boolean requested = false, recovered = false;
			while (!closed) {
				try {
					if (ArcticClient.config() != null && !ArcticClient.config().replayVideoEnabled) {
						state = "Gameplay video off"; Thread.sleep(1000); continue;
					}
					File tool = encoder.get();
					if (tool != null && ready == null) {
						codec = LiveVideoSession.chooseCodec(encoder, tool);
						if (!recovered) { recovered = true; LiveVideoSession.recover(root, videos, encoder, tool, LiveVideo::notice); }
						ready = tool; state = "Gameplay video ready";
					}
					if (ready != null) state = "Gameplay video ready";
					if (tool == null) {
						state = "downloading".equals(encoder.state()) ? "Preparing video encoder · " + Math.round(encoder.progress() * 100) + "%"
								: "Gameplay video needs FFmpeg · Get video encoder below";
						if (!requested && "missing".equals(encoder.state()) && System.getProperty("os.name", "").toLowerCase().contains("win")) {
							requested = true; encoder.download();
						}
					}
					Thread.sleep(1000);
				} catch (InterruptedException stop) { return; }
				catch (Exception error) { state = "Gameplay video unavailable: " + error.getMessage(); }
			}
		}, "arctic-video-prepare");
		prepare.setDaemon(true); prepare.setPriority(Thread.MIN_PRIORITY); prepare.start();
		shutdown = new Thread(() -> {
			closed = true; prepare.interrupt();
			LiveVideoSession active = session;
			if (active != null) active.finish();
			if (active != null) active.awaitFinish(4000);
			saves.shutdown();
			try { saves.awaitTermination(4, java.util.concurrent.TimeUnit.SECONDS); } catch (InterruptedException ignored) {}
		}, "arctic-video-close");
		Runtime.getRuntime().addShutdownHook(shutdown);
	}

	private static void notice(String message) {
		if (ArcticClient.platform() != null) ArcticClient.platform().log(false, "gameplay video: " + message);
		Notices.post("Gameplay video", message);
	}
	public Ffmpeg encoder() { return encoder; }
	public String status() {
		if (captureFailure != null) return captureFailure;
		LiveVideoSession active = session;
		return active == null ? state : active.error() != null ? "Video stopped: " + active.error() : "Capturing gameplay · " + active.description();
	}
	public int pending() { return LiveVideoSession.pending(); }
	public boolean capturing() { LiveVideoSession s = session; return s != null && s.error() == null; }
	void close() {
		closed = true; prepare.interrupt();
		LiveVideoSession active = session; if (active != null) { active.finish(); active.awaitFinish(4000); }
		saves.shutdown();
		try { Runtime.getRuntime().removeShutdownHook(shutdown); } catch (IllegalStateException ignored) {}
	}

	/** Game thread, after the HUD has been drawn. Never waits for a pipe, encoder, or file. */
	void frame(Recorder.Take take, ReplayBackend backend, boolean playing, boolean enabled, int height, int fps, int seconds) {
		if (!playing || !enabled || take == null || backend == null || backend.exportBlocked() != null) {
			stop(); return;
		}
		if (take != lastTake) { stop(); lastTake = take; stoppedForTake = false; captureFailure = null; }
		LiveVideoSession active = session;
		if (active != null) active.retain(seconds);
		if (active != null && active.error() != null) { stoppedForTake = true; return; }
		if (ready == null || stoppedForTake) return;
		final int captureFps = active == null ? fps : active.fps();
		final int captureHeight = active == null ? height : active.height();
		long now = System.nanoTime();
		if (now < nextFrame || (active != null && !active.hasRoom()) || !capturing.compareAndSet(false, true)) return;
		// Back off the readback rate on slow GPUs instead of blocking gameplay for video.
		nextFrame = now + Math.max(1_000_000_000L / captureFps, captureCost * 12);
		final Recorder.Take owner = take;
		final long before = System.nanoTime();
		try {
			backend.captureLive((w, h, rgba, bottomUp) -> {
				long copied = System.nanoTime();
				try {
					if (owner != lastTake) return;
					LiveVideoSession current = session;
					if (current == null) {
						int[] size = size(w, h, captureHeight);
						current = new LiveVideoSession(root, videos, encoder, ready, codec, backend, size[0], size[1], captureFps, saves, LiveVideo::notice);
						session = current;
						current.retain(seconds);
						synchronized (sounds) { for (AudioEvent e : sounds.values()) current.audio(e.at(owner.time(), 0)); }
					}
					current.offer(w, h, rgba, bottomUp);
				} catch (RuntimeException error) {
					captureFailed(owner, error);
				} finally { captureCost = (captureCost * 3 + System.nanoTime() - copied) / 4; capturing.set(false); }
			}, captureHeight);
			long call = System.nanoTime() - before;
			captureCost = active == null ? Math.min(call, 2_000_000L) : (captureCost * 7 + call) / 8;
		} catch (RuntimeException error) {
			capturing.set(false); captureFailed(owner, error);
		}
	}
	private void captureFailed(Recorder.Take owner, RuntimeException error) {
		stop(); lastTake = owner; stoppedForTake = true;
		captureFailure = "Video capture unavailable: " + error.getMessage(); notice(captureFailure + ". 3D recording continues.");
	}

	public static int[] size(int w, int h, int height) {
		double scale = Math.min(1, Math.min(height * 16.0 / (9 * w), (double) height / h));
		return new int[]{Math.max(2, (int) (w * scale) & ~1), Math.max(2, (int) (h * scale) & ~1)};
	}
	void audio(AudioEvent event) {
		synchronized (sounds) { if (event.resource.isEmpty()) sounds.remove(event.id); else if (sounds.size() < 512 || sounds.containsKey(event.id)) sounds.put(event.id, event); }
		LiveVideoSession s = session; if (s != null) s.audio(event);
	}
	void resetAudio() { synchronized (sounds) { sounds.clear(); } }
	boolean clip(int seconds) {
		LiveVideoSession s = session;
		if (s == null || s.error() != null) { notice("Video buffer isn't ready yet. " + status()); return false; }
		s.clip(seconds); return true;
	}
	public void startRecording() { LiveVideoSession s = session; if (s != null && s.error() == null) s.startRecording(); else notice("Video buffer isn't ready yet. " + status()); }
	public void stopRecording() { LiveVideoSession s = session; if (s != null) s.stopRecording(); }
	private void stop() {
		LiveVideoSession old = session; session = null; lastTake = null; nextFrame = 0;
		if (old != null) old.finish();
	}
}
