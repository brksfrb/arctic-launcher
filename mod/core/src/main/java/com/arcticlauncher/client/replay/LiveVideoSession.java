package com.arcticlauncher.client.replay;

import com.google.gson.Gson;
import java.io.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;

/** A bounded frame mailbox feeding one real-time encoder; saved clips only remux compressed segments. */
final class LiveVideoSession {
	private static final Gson JSON = new Gson();
	private static final AtomicInteger PENDING = new AtomicInteger();
	private static final long MIN_FREE = 256L << 20;
	private final File dir, videos, tool;
	private final Ffmpeg encoder;
	private final String codec;
	private final ReplayBackend backend;
	private final int width, height, fps;
	private final long started = System.nanoTime();
	private final ExecutorService saves;
	private final Consumer<String> notice;
	private final BlockingQueue<byte[]> available = new ArrayBlockingQueue<byte[]>(3);
	private final BlockingQueue<byte[]> frames = new ArrayBlockingQueue<byte[]>(2);
	private final BlockingQueue<AudioEvent> audioEvents = new ArrayBlockingQueue<AudioEvent>(2048);
	private final List<Job> jobs = new ArrayList<Job>();
	private volatile boolean finishing, finished;
	private volatile double finishAt;
	private volatile String failure;
	private volatile Process process;
	private volatile ServerSocket audioServer;
	private volatile Socket audioSocket;
	private volatile double encodedTo;
	private volatile long writeAt = System.nanoTime();
	private Job manual;
	private volatile int retentionSeconds = 33;
	private volatile double oldest;
	private final List<Segment> index = new ArrayList<Segment>();
	private RandomAccessFile listing;

	LiveVideoSession(File root, File videos, Ffmpeg encoder, File tool, String codec, ReplayBackend backend,
			int width, int height, int fps, ExecutorService saves, Consumer<String> notice) {
		this.dir = new File(root, UUID.randomUUID().toString()); this.videos = videos;
		this.encoder = encoder; this.tool = tool; this.codec = codec; this.backend = backend;
		this.width = width; this.height = height; this.fps = fps; this.saves = saves; this.notice = notice;
		for (int i = 0; i < 3; i++) available.add(new byte[width * height * 3]);
		Thread writer = new Thread(this::write, "arctic-gameplay-video");
		writer.setDaemon(true); writer.setPriority(Thread.MIN_PRIORITY); writer.start();
	}
	static int pending() { return PENDING.get(); }
	String description() { return height + "p / " + fps + " FPS · " + (codec.equals("libx264") ? "2 encoder threads" : "hardware encoder"); }
	String error() { return failure; }
	int fps() { return fps; }
	int height() { return height; }
	void retain(int seconds) { retentionSeconds = Math.max(2, Math.min(120, seconds)) + 3; }
	boolean hasRoom() { return !available.isEmpty() && frames.remainingCapacity() > 0 && !finishing; }
	private double seconds() { return (System.nanoTime() - started) / 1_000_000_000.0; }

	/** CPU copy is bounded by output resolution. Resize changes letterbox into the original video size. */
	void offer(int w, int h, ByteBuffer rgba, boolean bottomUp) {
		byte[] rgb = available.poll();
		if (rgb == null || finishing) { if (rgb != null) available.offer(rgb); return; }
		double scale = Math.min((double) width / w, (double) height / h);
		int dw = Math.max(1, (int) (w * scale)), dh = Math.max(1, (int) (h * scale));
		int left = (width - dw) / 2, top = (height - dh) / 2, base = rgba.position();
		Arrays.fill(rgb, (byte) 0);
		for (int y = 0; y < dh; y++) {
			int sy = y * h / dh, row = base + (bottomUp ? h - 1 - sy : sy) * w * 4;
			int out = ((top + y) * width + left) * 3;
			for (int x = 0; x < dw; x++) {
				int at = row + x * w / dw * 4;
				rgb[out++] = rgba.get(at); rgb[out++] = rgba.get(at + 1); rgb[out++] = rgba.get(at + 2);
			}
		}
		if (!frames.offer(rgb)) available.offer(rgb);
	}
	void audio(AudioEvent event) {
		if (finishing) return;
		AudioEvent copy = event.at(event.time, 0); copy.time = (int) (seconds() * 1000);
		if (!audioEvents.offer(copy)) fail("The video encoder couldn't keep up with game audio");
	}
	void clip(int seconds) { double to = seconds(); save(new Job(Math.max(oldest, Math.max(0, to - seconds)), to, output("Clip"))); }
	synchronized void startRecording() {
		if (manual != null) return;
		manual = new Job(seconds(), -1, output("Recording"));
		Job begun = manual;
		saves.execute(() -> { try { persist(begun); } catch (IOException e) { fail(e.getMessage()); } });
	}
	synchronized void stopRecording() {
		if (manual == null) return;
		Job done = manual; manual = null; done.to = seconds(); save(done);
	}
	private String output(String kind) {
		return kind + " " + new java.text.SimpleDateFormat("yyyy-MM-dd HH-mm-ss").format(new Date()) + " " + UUID.randomUUID().toString().substring(0, 6) + ".mp4";
	}
	private void save(Job job) {
		synchronized (this) { jobs.add(job); }
		PENDING.incrementAndGet();
		saves.execute(() -> {
			try {
				persist(job);
				List<Segment> selected;
				long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
				for (;;) {
					List<Segment> all = indexedSegments();
					if (!all.isEmpty() && all.get(all.size() - 1).to >= job.to - .1) { selected = select(all, job); break; }
					if (finished || failure != null || System.nanoTime() > deadline) throw new IOException("Video stopped before this moment finished saving. Your 3D replay is still available.");
					Thread.sleep(50);
				}
				remux(dir, videos, encoder, tool, selected, job);
				jobFile(job).delete(); notice.accept("Saved " + job.output + " · replay_videos");
			} catch (Exception e) { notice.accept("Couldn't save MP4: " + e.getMessage()); }
			finally { synchronized (this) { jobs.remove(job); } PENDING.decrementAndGet(); }
		});
	}
	void finish() { if (!finishing) { stopRecording(); finishAt = seconds(); finishing = true; } }
	void awaitFinish(long ms) { long end = System.currentTimeMillis() + ms; while (!finished && System.currentTimeMillis() < end) try { Thread.sleep(25); } catch (InterruptedException e) { return; } }
	private void fail(String reason) {
		if (failure != null) return;
		failure = reason; finish();
		Process p = process; if (p != null) p.destroy();
		notice.accept("Capture stopped: " + reason + ". 3D replay recording continues.");
	}

	private void write() {
		Thread audio = null;
		try {
			if (!dir.mkdirs() && !dir.isDirectory()) throw new IOException("Could not create video buffer");
			Files.write(new File(dir, ".owner").toPath(), "arctic-live-video-v1".getBytes(StandardCharsets.UTF_8));
			audioServer = new ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1")); audioServer.setSoTimeout(15000);
			List<String> args = new ArrayList<String>(Arrays.asList(tool.getAbsolutePath(), "-y", "-loglevel", "error",
					"-thread_queue_size", "8", "-probesize", "32", "-analyzeduration", "0", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", width + "x" + height, "-r", "" + fps, "-i", "pipe:0",
					"-thread_queue_size", "8", "-probesize", "32", "-analyzeduration", "0", "-f", "s16le", "-ar", "48000", "-ac", "2", "-i", "tcp://127.0.0.1:" + audioServer.getLocalPort(),
					"-c:v", codec));
			args.addAll(codecFlags(codec));
			args.addAll(Arrays.asList("-pix_fmt", "yuv420p", "-g", "" + fps, "-bf", "0", "-force_key_frames", "expr:gte(t,n_forced*1)",
					"-c:a", "aac", "-b:a", "96k", "-f", "segment", "-segment_time", "1", "-reset_timestamps", "1",
					"-segment_list", new File(dir, "segments.csv").getAbsolutePath(), "-segment_list_type", "csv",
					new File(dir, "part-%08d.mkv").getAbsolutePath()));
			process = encoder.command(args).redirectErrorStream(true).start();
			ByteArrayOutputStream errors = new ByteArrayOutputStream();
			Thread drain = new Thread(() -> {
				try (InputStream in = process.getInputStream()) { byte[] b = new byte[1024]; int n; while ((n = in.read(b)) >= 0) synchronized (errors) { if (errors.size() < 4096) errors.write(b, 0, Math.min(n, 4096 - errors.size())); } }
				catch (IOException ignored) {}
			}, "arctic-capture-log"); drain.setDaemon(true); drain.start();
			audio = new Thread(this::writeAudio, "arctic-capture-audio"); audio.setDaemon(true); audio.setPriority(Thread.MIN_PRIORITY); audio.start();
			Thread watchdog = new Thread(() -> {
				while (!finished) {
					if (System.nanoTime() - writeAt > TimeUnit.SECONDS.toNanos(15)) { fail("The video encoder stopped responding"); return; }
					try { Thread.sleep(1000); } catch (InterruptedException e) { return; }
				}
			}, "arctic-capture-watch"); watchdog.setDaemon(true); watchdog.start();
			byte[] last = null;
			long index = 0, check = 0;
			try (OutputStream out = new BufferedOutputStream(process.getOutputStream(), 64 * 1024)) {
				while (failure == null) {
					byte[] newest;
					while ((newest = frames.poll()) != null) { if (last != null) available.offer(last); last = newest; }
					if (finishing && index >= Math.max(1, Math.ceil(finishAt * fps))) break;
					if (last == null || index > seconds() * fps) { Thread.sleep(2); continue; }
					out.write(last); out.flush();
					encodedTo = (double) ++index / fps; writeAt = System.nanoTime();
					if (index - check >= fps * 2) { check = index; prune(); if (dir.getUsableSpace() < MIN_FREE) throw new IOException("Not enough free disk space for video capture"); }
				}
			}
			if (!process.waitFor(10, TimeUnit.SECONDS)) { process.destroyForcibly(); throw new IOException("Video encoder did not finish"); }
			if (process.exitValue() != 0 && failure == null) {
				String detail; synchronized (errors) { detail = new String(errors.toByteArray(), StandardCharsets.UTF_8); }
				throw new IOException("Video encoder failed: " + detail.trim());
			}
		} catch (Exception e) { fail(e.getMessage()); }
		finally {
			if (audioServer != null) try { audioServer.close(); } catch (IOException ignored) {}
			if (audioSocket != null) try { audioSocket.close(); } catch (IOException ignored) {}
			Process p = process; if (p != null && p.isAlive()) p.destroy();
			synchronized (this) {
				try { indexedSegments(); } catch (IOException ignored) {}
				finished = true;
				if (listing != null) try { listing.close(); } catch (IOException ignored) {}
			}
			saves.execute(() -> {
				File[] pending = dir.listFiles((d, name) -> name.startsWith("save-") && name.endsWith(".json"));
				if (pending != null && pending.length == 0) cleanBuffer(dir);
			});
		}
	}
	private void writeAudio() {
		try (Socket socket = audioServer.accept(); AudioMix mix = new AudioMix(backend, 0, audioEvents::poll)) {
			audioSocket = socket;
			OutputStream out = socket.getOutputStream();
			long index = 0;
			while (failure == null) {
				if (finishing && index >= Math.max(1, Math.ceil(finishAt * fps))) break;
				if (index > seconds() * fps) { Thread.sleep(2); continue; }
				int count = (int) (((index + 1) * AudioMix.RATE / fps) - (index * AudioMix.RATE / fps));
				out.write(mix.block(count)); index++;
			}
		} catch (Exception e) { if (!finished && failure == null) fail("Game audio capture failed: " + e.getMessage()); }
	}
	private synchronized void prune() throws IOException {
		double before = Math.max(0, encodedTo - retentionSeconds);
		if (manual != null) before = Math.min(before, manual.from);
		for (Job job : jobs) before = Math.min(before, job.from);
		indexedSegments();
		Iterator<Segment> it = index.iterator();
		while (it.hasNext()) { Segment s = it.next(); if (s.to < before && s.file.delete()) it.remove(); }
		if (!index.isEmpty()) oldest = index.get(0).from;
	}
	/** Read each finalized segment once, keeping the live index bounded by retention. */
	private synchronized List<Segment> indexedSegments() throws IOException {
		File csv = new File(dir, "segments.csv");
		if (listing == null && csv.isFile()) listing = new RandomAccessFile(csv, "r");
		if (listing != null && !finished) {
			while (listing.getFilePointer() < listing.length()) {
				long start = listing.getFilePointer(); String line = listing.readLine(); long end = listing.getFilePointer();
				if (end > start) { listing.seek(end - 1); if (listing.read() != '\n') { listing.seek(start); break; } }
				Segment s = parseSegment(dir, line); if (s != null) index.add(s);
			}
		}
		return new ArrayList<Segment>(index);
	}
	private File jobFile(Job job) { return new File(dir, "save-" + job.id + ".json"); }
	private void persist(Job job) throws IOException {
		dir.mkdirs(); File path = jobFile(job), part = new File(path + ".part");
		Files.write(part.toPath(), JSON.toJson(job).getBytes(StandardCharsets.UTF_8));
		Files.move(part.toPath(), path.toPath(), StandardCopyOption.REPLACE_EXISTING);
	}
	static final class Job {
		String id = UUID.randomUUID().toString(), output;
		double from, to;
		Job(double from, double to, String output) { this.from = from; this.to = to; this.output = output; }
	}
	static final class Segment {
		final File file; final double from, to;
		Segment(File file, double from, double to) { this.file = file; this.from = from; this.to = to; }
	}
	static List<Segment> segments(File dir) throws IOException {
		File csv = new File(dir, "segments.csv");
		List<Segment> all = new ArrayList<Segment>();
		if (!csv.isFile()) return all;
		try (BufferedReader in = Files.newBufferedReader(csv.toPath(), StandardCharsets.UTF_8)) {
			String line;
			while ((line = in.readLine()) != null) {
				Segment s = parseSegment(dir, line); if (s != null) all.add(s);
			}
		}
		return all;
	}
	private static Segment parseSegment(File dir, String line) {
		if (line == null) return null; String[] parts = line.split(",");
		if (parts.length != 3) return null; String name = parts[0].replace("\"", "");
		if (!name.matches("part-[0-9]+\\.mkv")) return null;
		try { File file = new File(dir, name); return file.isFile() ? new Segment(file, Double.parseDouble(parts[1]), Double.parseDouble(parts[2])) : null; }
		catch (NumberFormatException ignored) { return null; }
	}
	static List<Segment> select(List<Segment> all, Job job) throws IOException {
		List<Segment> selected = new ArrayList<Segment>();
		for (Segment s : all) if (s.to > job.from && s.from < job.to) selected.add(s);
		if (selected.isEmpty() || selected.get(0).from > job.from + .1) throw new IOException("The requested screen video is no longer in the buffer");
		return selected;
	}
	static void remux(File dir, File videos, Ffmpeg encoder, File tool, List<Segment> selected, Job job) throws Exception {
		if (job.output == null || !job.output.equals(new File(job.output).getName()) || !job.output.endsWith(".mp4")) throw new IOException("Invalid saved video name");
		videos.mkdirs(); File out = new File(videos, job.output), part = new File(videos, job.output + ".part.mp4");
		File list = new File(dir, "concat-" + job.id + ".txt");
		StringBuilder text = new StringBuilder();
		// Explicit durations let the concat demuxer seek to the preceding keyframe.
		// Keeping that preroll in MP4's edit list avoids a black first second without re-encoding.
		for (Segment s : selected) text.append("file '").append(s.file.getAbsolutePath().replace("\\", "/").replace("'", "'\\''"))
				.append("'\nduration ").append(s.to - s.from).append('\n');
		Files.write(list.toPath(), text.toString().getBytes(StandardCharsets.UTF_8));
		List<String> args = Arrays.asList(tool.getAbsolutePath(), "-y", "-loglevel", "error", "-ss", "" + Math.max(0, job.from - selected.get(0).from),
				"-f", "concat", "-safe", "0", "-i", list.getAbsolutePath(), "-t", "" + Math.max(.04, job.to - job.from),
				"-map", "0:v:0", "-map", "0:a:0", "-c", "copy", "-avoid_negative_ts", "disabled", "-use_editlist", "1", "-movflags", "+faststart", part.getAbsolutePath());
		Process p = encoder.command(args).redirectOutput(ProcessBuilder.Redirect.INHERIT).redirectError(ProcessBuilder.Redirect.INHERIT).start();
		try {
			long bytes = 0; for (Segment segment : selected) bytes += segment.file.length();
			// Long recordings can require several passes over gigabytes of compressed data.
			// Bound time by the actual copy workload rather than treating them like short clips.
			long waitSeconds = 30 + bytes / (2L * 1024 * 1024);
			if (!p.waitFor(waitSeconds, TimeUnit.SECONDS)) { p.destroyForcibly(); throw new IOException("MP4 finalization timed out"); }
			if (p.exitValue() != 0 || part.length() == 0) throw new IOException("MP4 finalization failed");
			Files.move(part.toPath(), out.toPath(), StandardCopyOption.REPLACE_EXISTING);
		} finally { list.delete(); part.delete(); }
	}
	static void recover(File root, File videos, Ffmpeg encoder, File tool, Consumer<String> notice) {
		File[] sessions = root.listFiles(File::isDirectory); if (sessions == null) return;
		for (File dir : sessions) {
			if (!dir.getName().matches("[0-9a-f-]{36}") || !new File(dir, ".owner").isFile()) continue;
			File[] pending = dir.listFiles((d, name) -> name.startsWith("save-") && name.endsWith(".json"));
			if (pending == null) continue;
			for (File file : pending) try {
				Job job = JSON.fromJson(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8), Job.class);
				List<Segment> all = segments(dir);
				if (job.to < 0 && !all.isEmpty()) job.to = all.get(all.size() - 1).to;
				if (all.isEmpty() || all.get(all.size() - 1).to < job.to - .1) throw new IOException("Capture ended before the clip completed");
				remux(dir, videos, encoder, tool, select(all, job), job); file.delete(); notice.accept("Recovered " + job.output);
			} catch (Exception e) { notice.accept("Saved video needs attention: " + e.getMessage()); }
			// Keep unfinished saved jobs, remove only this recorder's disposable compressed buffer.
			File[] remaining = dir.listFiles((d, name) -> name.startsWith("save-") && name.endsWith(".json"));
			if (remaining != null && remaining.length == 0) cleanBuffer(dir);
		}
	}
	private static void cleanBuffer(File dir) {
		if (!new File(dir, ".owner").isFile()) return;
		File[] files = dir.listFiles(); if (files == null) return;
		for (File f : files) if (f.isFile() && (f.getName().matches("part-[0-9]+\\.mkv") || f.getName().equals("segments.csv")
				|| f.getName().matches("concat-[0-9a-f-]+\\.txt"))) f.delete();
		File marker = new File(dir, ".owner"); marker.delete(); dir.delete();
	}
	static List<String> codecFlags(String codec) {
		if (codec.equals("h264_nvenc")) return Arrays.asList("-preset", "p1", "-rc", "vbr", "-cq", "26", "-b:v", "2500k", "-maxrate", "5M", "-bufsize", "10M");
		if (codec.equals("h264_amf")) return Arrays.asList("-quality", "speed", "-b:v", "2500k", "-maxrate", "5M");
		if (codec.equals("h264_videotoolbox")) return Arrays.asList("-realtime", "1", "-b:v", "2500k");
		if (codec.equals("h264_qsv")) return Arrays.asList("-preset", "veryfast", "-b:v", "2500k", "-maxrate", "5M");
		return Arrays.asList("-threads", "2", "-preset", "ultrafast", "-crf", "26", "-maxrate", "5M", "-bufsize", "10M");
	}
	static String chooseCodec(Ffmpeg encoder, File tool) {
		String os = System.getProperty("os.name", "").toLowerCase();
		String[] candidates = os.contains("mac") ? new String[]{"h264_videotoolbox"} : new String[]{"h264_nvenc", "h264_amf", "h264_qsv"};
		for (String codec : candidates) {
			List<String> args = new ArrayList<String>(Arrays.asList(tool.getAbsolutePath(), "-v", "error", "-f", "lavfi", "-i", "color=s=128x128:r=30", "-frames:v", "1", "-c:v", codec));
			args.addAll(codecFlags(codec)); args.addAll(Arrays.asList("-f", "null", "-"));
			Process p = null;
			try {
				p = encoder.command(args).redirectOutput(new File(os.contains("win") ? "NUL" : "/dev/null")).redirectError(new File(os.contains("win") ? "NUL" : "/dev/null")).start();
				if (p.waitFor(3, TimeUnit.SECONDS) && p.exitValue() == 0) return codec;
			} catch (Exception ignored) {}
			finally { if (p != null && p.isAlive()) p.destroyForcibly(); }
		}
		return "libx264";
	}
}
