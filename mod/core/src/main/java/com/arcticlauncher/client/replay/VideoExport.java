package com.arcticlauncher.client.replay;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.channels.Channels;
import java.nio.channels.WritableByteChannel;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Turns a stretch of a replay into an MP4: the game draws each frame at an
 * exact time step, and the frames stream into FFmpeg on a background thread
 * (the game waits only when FFmpeg falls behind).
 */
public final class VideoExport implements ReplayBackend.FrameSink {
	/** Frames waiting for FFmpeg at most (each a full screen of pixels). */
	private static final int QUEUE = 2;
	private static final long FINISH_WAIT_MS = 10_000;

	private final File ffmpeg;
	private final File output;
	private final int from;
	private final int to;
	private final int fps;
	private final BlockingQueue<ByteBuffer> frames = new ArrayBlockingQueue<ByteBuffer>(QUEUE);
	private final BlockingQueue<ByteBuffer> spare = new ArrayBlockingQueue<ByteBuffer>(QUEUE + 2);
	private final Runnable onDone;
	private int index;
	private volatile int requested;
	private volatile int delivered;
	private volatile boolean finishing;
	private volatile boolean cancelled;
	private volatile String failed;
	private volatile boolean done;
	private Process process;
	private Thread writer;
	private File audioReplay;
	private ReplayBackend audioBackend;
	private boolean interleavedAudio;
	private java.net.ServerSocket audioServer;
	private volatile java.net.Socket audioSocket;
	private Thread audioWriter;
	void audio(File replay, ReplayBackend backend) { audioReplay = replay; audioBackend = backend; }
	private int audioFrames(long frame) {
		return (int) (((frame + 1) * AudioMix.RATE / fps) - (frame * AudioMix.RATE / fps));
	}
	private int width;
	private int height;

	/** {@code [from, to]} of the replay (ms) at {@code fps}; {@code onDone} runs when the file is complete. */
	public VideoExport(File ffmpeg, File output, int from, int to, int fps, Runnable onDone) {
		this.ffmpeg = ffmpeg;
		this.output = output;
		this.from = from;
		this.to = Math.max(from + 1, to);
		this.fps = fps;
		this.onDone = onDone;
	}

	int from() {
		return from;
	}

	double frameMillis() {
		return 1000.0 / fps;
	}

	/** The replay time of the frame being drawn. */
	double frameTime() {
		return from + index * frameMillis();
	}

	/** Frame drawn and grabbed: on to the next; false when past the end. */
	boolean advance() {
		requested++;
		index++;
		return !cancelled && failed == null && frameTime() <= to;
	}

	public float progress() {
		return (float) Math.min(1, (frameTime() - from) / (to - from));
	}

	public File output() {
		return output;
	}

	String failed() {
		return failed;
	}

	public boolean done() {
		return done;
	}

	boolean readyForFrame() { return delivered >= requested; }

	/** At most 720p, preserving aspect ratio and even encoder dimensions; never upscale. */
	static int[] outputSize(int w, int h) {
		double scale = Math.min(1, Math.min(1280.0 / w, 720.0 / h));
		return new int[] {Math.max(2, ((int) (w * scale)) & ~1), Math.max(2, ((int) (h * scale)) & ~1)};
	}

	/** A grabbed frame (render thread): copied, then queued for FFmpeg. */
	@Override
	public void frame(int w, int h, ByteBuffer rgba, boolean bottomUp) {
		if (cancelled || failed != null) {
			delivered++;
			return;
		}
		try {
			if (process == null) {
				int[] size = outputSize(w, h);
				startFfmpeg(size[0], size[1]);
			}
			ByteBuffer copy = spare.poll();
			if (copy == null || copy.capacity() < width * height * 4) {
				copy = ByteBuffer.allocateDirect(width * height * 4);
			}
			copy.clear();
			copy.order(rgba.order());
			int base = rgba.position();
			for (int y = 0; y < height; y++) {
				int sy = y * h / height;
				int row = base + (bottomUp ? h - 1 - sy : sy) * w * 4;
				for (int x = 0; x < width; x++) copy.putInt(rgba.getInt(row + (x * w / width) * 4));
			}
			copy.flip();
			while (!cancelled && failed == null && !frames.offer(copy, 100, TimeUnit.MILLISECONDS)) { /* bounded backpressure */ }
		} catch (IOException e) {
			failed = "The video encoder didn't start: " + e.getMessage();
			closeAudio();
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		} finally {
			delivered++;
		}
	}

	private void startFfmpeg(int w, int h) throws IOException {
		width = w;
		height = h;
		output.getParentFile().mkdirs();
		List<String> cmd = new ArrayList<String>();
		Ffmpeg finder = Replays.ffmpeg();
		interleavedAudio = audioReplay != null && finder != null && finder.builtin(ffmpeg);
		if (audioReplay != null && !interleavedAudio) {
			audioServer = new java.net.ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1"));
			audioServer.setSoTimeout(60_000);
		}
		String[] args = finder != null && finder.builtin(ffmpeg) ? new String[] {
				// Arctic Launcher's encoder: RGBA frames in, MP4 out.
				ffmpeg.getAbsolutePath(), "--encode-video", Integer.toString(w), Integer.toString(h),
				Integer.toString(fps), output.getAbsolutePath(),
		} : new String[] {
				ffmpeg.getAbsolutePath(), "-y", "-loglevel", "error",
				"-f", "rawvideo", "-pix_fmt", "rgba", "-s", w + "x" + h, "-r", Integer.toString(fps), "-i", "-",
				"-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2",
				"-filter_threads", "1", "-c:v", "libx264", "-threads", "2", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p",
				"-movflags", "+faststart", output.getAbsolutePath(),
		};
		for (String a : args) cmd.add(a);
		if (interleavedAudio) cmd.add("--audio");
		else if (audioServer != null) {
			int inputEnd = cmd.indexOf("-vf");
			cmd.addAll(inputEnd, java.util.Arrays.asList("-f", "s16le", "-ar", "48000", "-ac", "2", "-i",
					"tcp://127.0.0.1:" + audioServer.getLocalPort(), "-c:a", "aac", "-b:a", "96k"));
		}
		process = new ProcessBuilder(cmd).redirectErrorStream(true).start();
		drain(process.getInputStream());
		if (audioServer != null) {
			audioWriter = new Thread(() -> writeAudio(), "arctic-video-audio");
			audioWriter.setDaemon(true); audioWriter.setPriority(Thread.MIN_PRIORITY + 1); audioWriter.start();
		}
		final WritableByteChannel out = Channels.newChannel(process.getOutputStream());
		writer = new Thread(() -> write(out), "arctic-video");
		writer.setDaemon(true);
		writer.setPriority(Thread.MIN_PRIORITY + 1);
		writer.start();
	}

	/** FFmpeg's own messages (read so its pipe never fills). */
	private static void drain(final InputStream in) {
		Thread t = new Thread(() -> {
			byte[] buf = new byte[4096];
			try {
				while (in.read(buf) >= 0) {
					// Discarded; errors show as a bad exit code.
				}
			} catch (IOException ignored) {
				// FFmpeg closed.
			}
		}, "arctic-video-log");
		t.setDaemon(true);
		t.start();
	}

	private void write(WritableByteChannel out) {
		long written = 0;
		try (AudioMix audio = interleavedAudio ? new AudioMix(audioReplay, audioBackend, from) : null) {
			while (true) {
				ByteBuffer f = frames.poll(200, TimeUnit.MILLISECONDS);
				if (f != null) {
					while (f.hasRemaining()) {
						out.write(f);
					}
					if (audio != null) {
						ByteBuffer pcm = ByteBuffer.wrap(audio.block(audioFrames(written)));
						while (pcm.hasRemaining()) out.write(pcm);
					}
					written++;
					spare.offer(f);
					continue;
				}
				if (cancelled || (finishing && delivered >= requested && frames.isEmpty())) {
					break;
				}
			}
		} catch (IOException e) {
			if (!cancelled && failed == null) {
				failed = "The video encoder stopped: " + e.getMessage();
			}
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		} finally {
			closeQuietly(out);
			complete();
		}
	}

	private void writeAudio() {
		try (java.net.ServerSocket server = audioServer;
				java.net.Socket socket = server.accept();
				AudioMix mix = new AudioMix(audioReplay, audioBackend, from)) {
			audioSocket = socket;
			java.io.OutputStream out = socket.getOutputStream();
			long total = (long) Math.floor((to - from) * (double) fps / 1000) + 1;
			for (long i = 0; i < total && !cancelled; i++) out.write(mix.block(audioFrames(i)));
		} catch (IOException e) {
			if (!cancelled && failed == null) {
				failed = "Game audio could not be exported: " + e.getMessage();
				process.destroy();
			}
		}
	}

	private void closeAudio() {
		try { if (audioServer != null) audioServer.close(); } catch (IOException ignored) {}
		try { if (audioSocket != null) audioSocket.close(); } catch (IOException ignored) {}
	}

	private void complete() {
		try {
			if (!process.waitFor(30, TimeUnit.SECONDS)) {
				failed = "The video encoder timed out.";
				process.destroyForcibly();
			}
			int code = process.waitFor();
			if (audioWriter != null) audioWriter.join(2000);
			if (code != 0 && !cancelled && failed == null) {
				failed = "The video encoder failed (code " + code + ")";
			}
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		}
		closeAudio();
		if (cancelled || failed != null) {
			output.delete();
		}
		done = true;
		if (onDone != null) {
			onDone.run();
		}
	}

	/** No more frames: let FFmpeg finish the file (in the background). */
	void finish() {
		finishing = true;
		if (process == null) {
			done = true;
			if (failed == null && !cancelled) {
				failed = "Nothing was recorded.";
			}
			if (onDone != null) {
				onDone.run();
			}
			return;
		}
		final long deadline = System.currentTimeMillis() + FINISH_WAIT_MS;
		Thread watchdog = new Thread(() -> {
			while (!done && System.currentTimeMillis() < deadline) {
				try {
					Thread.sleep(100);
				} catch (InterruptedException e) {
					return;
				}
			}
			if (!done) {
				if (delivered < requested) failed = "A replay frame could not be captured.";
				delivered = requested;
			}
		}, "arctic-video-wait");
		watchdog.setDaemon(true);
		watchdog.start();
	}

	void cancel() {
		cancelled = true;
		closeAudio();
		frames.clear();
		if (process != null) {
			process.destroy();
		}
	}

	public String error() {
		return failed;
	}

	public boolean cancelled() {
		return cancelled;
	}

	private static void closeQuietly(WritableByteChannel c) {
		try {
			c.close();
		} catch (IOException ignored) {
			// Already closed.
		}
	}
}
