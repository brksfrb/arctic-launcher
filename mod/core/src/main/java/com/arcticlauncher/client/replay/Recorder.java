package com.arcticlauncher.client.replay;

import com.google.gson.Gson;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.UUID;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.function.Consumer;
import java.util.function.IntSupplier;
import java.util.function.Supplier;
import java.util.zip.*;

/** A rolling replay buffer and an independent, explicitly started recording. Packet writes are serialized; immutable snapshots are saved separately. */
public final class Recorder {
	static final String PACKETS_ENTRY = "recording.tmcpr";
	static final String META_ENTRY = "metaData.json";
	static final String SELF_ENTRY = "arctic/self.bin";
	static final String EXTRAS_ENTRY = "arctic.json";
	static final String THUMB_ENTRY = "arctic/thumb.png";
	private static final int BUFFER = 1 << 18;
	private static final long MAX_PENDING_BYTES = 64L << 20;
	/** These limits count compressed bytes on disk, not inflated network traffic. */
	static final long MAX_RECORDING_BYTES = 512L << 20;
	static final long MIN_FREE_BYTES = 512L << 20;
	private static final int COMPACT_INTERVAL_MS = 10_000;
	private static final Gson GSON = new Gson();
	private final File tempDir;
	private final File savedDir;
	private final IntSupplier clipSeconds;
	private final Consumer<String> log;
	private final Supplier<ReplayBackend.Classifier> sorter;
	private final long maxBytes;
	private final long minFreeBytes;
	private final int compactInterval;
	private final java.util.function.LongSupplier nanoTime;
	private final LinkedBlockingQueue<Runnable> tasks = new LinkedBlockingQueue<Runnable>();
	private final java.util.concurrent.ExecutorService compactions = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-replay-compact"); t.setDaemon(true); t.setPriority(Thread.MIN_PRIORITY); return t;
	});
	private volatile Take current;
	private final java.util.concurrent.atomic.AtomicInteger pendingSaves = new java.util.concurrent.atomic.AtomicInteger();
	private final java.util.concurrent.ExecutorService saves = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-replay-save"); t.setDaemon(true); t.setPriority(Thread.MIN_PRIORITY); return t;
	});

	public Recorder(File gameDir, IntSupplier clipSeconds, Consumer<String> log, Supplier<ReplayBackend.Classifier> sorter) {
		this(gameDir, clipSeconds, log, sorter, MAX_RECORDING_BYTES, MIN_FREE_BYTES, COMPACT_INTERVAL_MS);
	}

	Recorder(File gameDir, IntSupplier clipSeconds, Consumer<String> log, Supplier<ReplayBackend.Classifier> sorter,
			long maxBytes, long minFreeBytes, int compactInterval) {
		this(gameDir, clipSeconds, log, sorter, maxBytes, minFreeBytes, compactInterval, System::nanoTime);
	}

	Recorder(File gameDir, IntSupplier clipSeconds, Consumer<String> log, Supplier<ReplayBackend.Classifier> sorter,
			long maxBytes, long minFreeBytes, int compactInterval, java.util.function.LongSupplier nanoTime) {
		this.nanoTime = nanoTime;
		this.tempDir = new File(gameDir, ".arctic/replay-temp");
		this.savedDir = new File(gameDir, "replay_recordings");
		this.clipSeconds = clipSeconds;
		this.log = log;
		this.sorter = sorter;
		this.maxBytes = maxBytes;
		this.minFreeBytes = minFreeBytes;
		this.compactInterval = compactInterval;
		File[] old = tempDir.listFiles();
		if (old != null) {
			for (File f : old) {
				f.delete();
			}
		}
		Thread writer = new Thread(() -> {
			while (true) {
				try {
					tasks.take().run();
				} catch (InterruptedException e) {
					return;
				} catch (Throwable t) {
					log.accept("replay writer: " + t);
				}
			}
		}, "arctic-replay-writer");
		writer.setDaemon(true);
		writer.setPriority(Thread.MIN_PRIORITY + 1);
		writer.start();
	}

	public File savedDir() { return savedDir; }
	File tempDir() { return tempDir; }
	void afterPackets(Runnable r) { tasks.add(r); }
	void afterWrites(Runnable r) { tasks.add(() -> compactions.execute(() -> tasks.add(() -> saves.execute(r)))); }
	public Take current() { return current; }
	public boolean recording() { Take t = current; return t != null && t.recording; }
	public boolean bufferReady() { Take t = current; return t != null && !t.failed && !t.ended; }
	/** Files being written or waiting for the save worker, including manual recordings. */
	public int pendingSaves() { return pendingSaves.get(); }
	public long recordingBytes() { Take t = current; return t == null ? 0 : t.manualDiskBytes; }
	public int recordingMillis() { Take t = current; return recording() ? Math.max(0, t.time() - t.manualFrom) : 0; }

	/** Always retain the login/bootstrap, so F9 and F10 can be used without reconnecting. */
	public synchronized Take begin(String server, boolean singleplayer, int protocol, String mcVersion) {
		if (current != null) current.end();
		Take take = new Take(server, singleplayer, protocol, mcVersion);
		current = take;
		tasks.add(() -> {
			try { take.ring = new Ring(); } catch (IOException e) { take.fail(e.toString()); }
		});
		return take;
	}

	public boolean saveMoment(Consumer<File> done) { return saveMoment(null, done); }
	public boolean saveMoment(byte[] thumbnail, Consumer<File> done) {
		Take t = current;
		return t != null && t.clip(thumbnail, done);
	}

	/** The callback receives the completed manual recording, including when leaving the world. */
	public boolean startRecording(Consumer<File> done) {
		Take t = current;
		return t != null && t.startRecording(done);
	}
	public boolean stopRecording() {
		Take t = current;
		return t != null && t.stopRecording();
	}

	public final class Take {
		private volatile String server;
		final boolean singleplayer;
		final int protocol;
		final String mcVersion;
		final long date = System.currentTimeMillis();
		private final long startNanos = nanoTime.getAsLong();
		private long pausedNanos;
		private long pausedAt = -1;
		private volatile boolean ended;
		private volatile boolean failed;
		private volatile boolean recording;
		private volatile int manualFrom;
		private volatile int latest;
		private volatile int selfId = -1;
		private volatile String selfName;
		private volatile UUID selfUuid;
		// Writer thread only. Ring and manual never share writable files.
		private Ring ring;
		private boolean compacting;
		private Stream manual;
		private Consumer<File> manualDone;
		private int manualBase;
		private int compactedAt;
		private int checkedAt = -1;
		private volatile long manualDiskBytes;
		private long pending;

		Take(String server, boolean singleplayer, int protocol, String mcVersion) {
			this.server = server == null ? "Unknown" : server;
			this.singleplayer = singleplayer;
			this.protocol = protocol;
			this.mcVersion = mcVersion;
		}
		public synchronized int time() {
			long now = pausedAt >= 0 ? pausedAt : nanoTime.getAsLong();
			return (int) ((now - startNanos - pausedNanos) / 1_000_000L);
		}
		public synchronized void paused(boolean paused) {
			if (paused && pausedAt < 0) pausedAt = nanoTime.getAsLong();
			else if (!paused && pausedAt >= 0) { pausedNanos += nanoTime.getAsLong() - pausedAt; pausedAt = -1; }
		}
		public void name(String name) { if (name != null && !name.isEmpty() && !singleplayer) server = name; }
		public void self(int id, String name, UUID uuid) { selfId = id; selfName = name; selfUuid = uuid; }

		/** Taking the same lock for enqueueing controls and packets makes start/stop boundaries exact. */
		public synchronized void packet(final byte[] data) {
			if (ended || failed || data == null) return;
			if (pending + data.length > MAX_PENDING_BYTES) {
				failed = true;
				tasks.add(() -> fail("replay data arrived faster than it could be processed (64 MB queued); recording stopped to protect game memory"));
				return;
			}
			final int t = time();
			pending += data.length;
			tasks.add(() -> {
				synchronized (Take.this) { pending -= data.length; }
				if (ring == null || failed) return;
				try {
					ring.active().packet(t, data);
					latest = t;
					if (manual != null) manual.packet(Math.max(0, t - manualBase), data);
					maintain(t);
				} catch (IOException | RuntimeException e) { fail(e.toString()); }
			});
		}

		public synchronized void sample(final SelfSample sample) {
			if (ended || failed) return;
			sample.time = time();
			tasks.add(() -> {
				if (ring == null || failed) return;
				try {
					ring.active().sample(sample);
					latest = sample.time;
					if (manual != null) {
						int t = sample.time;
						sample.time = Math.max(0, t - manualBase);
						manual.sample(sample);
						sample.time = t;
					}
					maintain(sample.time);
				} catch (IOException | RuntimeException e) { fail(e.toString()); }
			});
		}

		public synchronized void audio(final AudioEvent event) {
			if (ended || failed) return;
			tasks.add(() -> {
				if (ring == null || failed) return;
				try {
					ring.active().audio(event);
					if (manual != null) manual.audio(event.at(event.time, manualBase));
				} catch (IOException e) { fail(e.toString()); }
			});
		}

		private void maintain(int t) throws IOException {
			if (!compacting && t - compactedAt >= compactInterval && t > clipSeconds.getAsInt() * 1000) {
				ReplayBackend.Classifier classifier = sorter.get();
				if (classifier != null) {
					final Ring buffer = ring;
					final Snapshot source = buffer.view();
					final java.util.List<Stream> prefix = new java.util.ArrayList<Stream>(buffer.streams);
					final int from = Math.max(0, t - clipSeconds.getAsInt() * 1000);
					// Freeze the old segments; incoming packets immediately go to a fresh one.
					buffer.streams.add(new Stream());
					for (Stream stream : prefix) stream.retain();
					compacting = true;
					compactions.execute(() -> {
						Stream next = null;
						try {
							next = new Stream();
							copyRange(source, next.packets, next.self, from, t, 0, classifier, next);
							next.flush();
							final Stream compacted = next;
							tasks.add(() -> {
								if (ring == buffer) {
									buffer.streams.subList(0, prefix.size()).clear();
									buffer.streams.add(0, compacted);
								} else compacted.delete();
								for (Stream stream : prefix) stream.delete();
								compacting = false;
							});
						} catch (IOException | RuntimeException e) {
							if (next != null) next.delete();
							tasks.add(() -> { compacting = false; if (!ended && !failed) fail("buffer cleanup failed: " + e); });
						} finally { for (Stream stream : prefix) stream.release(); }
					});
				}
				compactedAt = t;
			}
			if (checkedAt < 0 || t - checkedAt >= 5_000) {
				checkedAt = t;
				ring.flush();
				if (manual != null) { manual.flush(); manualDiskBytes = manual.diskBytes(); }
				if (tempDir.getUsableSpace() < minFreeBytes) fail("less than 512 MB of disk space remains");
				else if (ring.diskBytes() > maxBytes) fail("the replay's world data is too large for the temporary buffer");
			}
		}

		private synchronized boolean clip(byte[] thumbnail, Consumer<File> done) {
			if (ended) return false;
			final int at = failed ? latest : time();
			final int from = Math.max(0, at - clipSeconds.getAsInt() * 1000);
			tasks.add(() -> saveLater(ring, from, at, "Clip", thumbnail, done));
			return true;
		}
		private synchronized boolean startRecording(Consumer<File> done) {
			if (ended || failed || recording) return false;
			final int at = time();
			recording = true;
			manualFrom = at;
			tasks.add(() -> {
				manualBase = at;
				manualDone = done;
				try {
					manual = new Stream();
					// Seed a fresh stream with the world at this instant, then append live packets.
					copyRange(ring.view(), manual.packets, manual.self, at, at, at, sorter.get(), manual);
				} catch (IOException | RuntimeException e) { fail(e.toString()); }
			});
			return true;
		}
		private synchronized boolean stopRecording() {
			if (!recording) return false;
			recording = false;
			final int at = time();
			tasks.add(() -> finishManual(at));
			return true;
		}
		private void finishManual(int at) {
			Stream stream = manual;
			Consumer<File> done = manualDone;
			manual = null;
			manualDone = null;
			saveLater(stream, 0, Math.max(0, at - manualBase), "Recording", null, done);
			if (stream != null) stream.delete();
		}
		public synchronized void end() {
			if (ended) return;
			ended = true;
			recording = false;
			final int at = time();
			if (current == this) current = null;
			tasks.add(() -> {
				if (manual != null) finishManual(at);
				if (ring != null) ring.delete();
				ring = null;
			});
		}
		private void fail(String why) {
			failed = true;
			recording = false;
			log.accept("replay: stopped: " + why);
			notice("Replay buffer stopped", why + ". Saved clips are safe.");
			if (manual != null) finishManual(time());
		}
		private void saveLater(Stream stream, int from, int to, String kind, byte[] thumbnail, Consumer<File> done) {
			saveLater(stream == null ? java.util.Collections.emptyList() : java.util.Collections.singletonList(stream), from, to, kind, thumbnail, done);
		}
		private void saveLater(Ring buffer, int from, int to, String kind, byte[] thumbnail, Consumer<File> done) {
			saveLater(buffer == null ? java.util.Collections.emptyList() : new java.util.ArrayList<Stream>(buffer.streams), from, to, kind, thumbnail, done);
		}
		private void saveLater(java.util.List<Stream> streams, int from, int to, String kind, byte[] thumbnail, Consumer<File> done) {
			if (streams.isEmpty()) { if (done != null) done.accept(null); return; }
			try {
				Snapshot view = snapshot(streams);
				for (Stream stream : streams) stream.retain();
				pendingSaves.incrementAndGet();
				saves.execute(() -> {
					try { File file = save(view, from, to, kind, thumbnail); if (done != null) done.accept(file); }
					finally {
						pendingSaves.decrementAndGet();
						for (Stream stream : streams) stream.release();
					}
				});
			} catch (IOException e) { log.accept("replay: snapshot failed: " + e); if (done != null) done.accept(null); }
		}
		private File save(Snapshot stream, int from, int to, String kind, byte[] thumbnail) {
			if (stream == null || stream.packetBytes == 0) return null;
			String where = server.replaceAll("[^A-Za-z0-9 ._-]", "_").trim();
			if (where.length() > 40) where = where.substring(0, 40).trim();
			String when = new SimpleDateFormat("yyyy-MM-dd HH-mm-ss-SSS").format(new Date());
			File out = new File(savedDir, (where.isEmpty() ? "Replay" : where) + " " + when + " " + kind + " " + UUID.randomUUID().toString().substring(0, 8) + ".mcpr");
			File part = new File(out.getPath() + ".part");
			try {
				savedDir.mkdirs();
				ReplayMeta meta = new ReplayMeta();
				meta.singleplayer = singleplayer; meta.serverName = server; meta.duration = to - from;
				meta.date = System.currentTimeMillis(); meta.protocol = protocol; meta.selfId = selfId; meta.mcversion = mcVersion;
				ReplayMeta.Extras extras = new ReplayMeta.Extras();
				extras.selfName = selfName; extras.selfUuid = selfUuid == null ? null : selfUuid.toString();
				extras.clipSeconds = clipSeconds.getAsInt(); extras.moments.add(to - from);
				try (ZipOutputStream zip = new ZipOutputStream(new BufferedOutputStream(new FileOutputStream(part), BUFFER))) {
					zip.setLevel(6);
					// Batch packet headers, entity updates and track fields before ZIP compression.
					// Buffering only the file underneath ZIP still calls zlib for every tiny write.
					DataOutputStream track = new DataOutputStream(new BufferedOutputStream(zip, BUFFER));
					entry(zip, META_ENTRY, GSON.toJson(meta).getBytes(StandardCharsets.UTF_8));
					entry(zip, EXTRAS_ENTRY, GSON.toJson(extras).getBytes(StandardCharsets.UTF_8));
					zip.putNextEntry(new ZipEntry(PACKETS_ENTRY));
					if (from == 0) {
						try (InputStream in = stream.packets()) { copyLength(in, track, stream.packetBytes); }
					} else copyRange(stream, track, null, from, to, from, sorter.get(), null);
					track.flush();
					zip.closeEntry();
					zip.putNextEntry(new ZipEntry(SELF_ENTRY));
					copySelf(stream, track, from, to, from, null);
					track.flush();
					zip.closeEntry();
					zip.putNextEntry(new ZipEntry(AudioEvent.ENTRY));
					copyAudio(stream, track, from, to, from, null);
					track.flush();
					zip.closeEntry();
					if (thumbnail != null) entry(zip, THUMB_ENTRY, thumbnail);
				}
				Files.move(part.toPath(), out.toPath(), StandardCopyOption.REPLACE_EXISTING);
				return out;
			} catch (IOException | RuntimeException e) {
				part.delete(); log.accept("replay: save failed: " + e); return null;
			}
		}
	}

	private static void notice(String title, String text) { com.arcticlauncher.client.notice.Notices.post(title, text); }

	/** Only the last segment is writable. Compaction replaces a frozen prefix without copying live traffic. */
	private final class Ring {
		final java.util.List<Stream> streams = new java.util.ArrayList<Stream>();
		Ring() throws IOException { streams.add(new Stream()); }
		Stream active() { return streams.get(streams.size() - 1); }
		Snapshot view() throws IOException { return snapshot(streams); }
		void flush() throws IOException { active().flush(); }
		long diskBytes() { long bytes = 0; for (Stream stream : streams) bytes += stream.diskBytes(); return bytes; }
		void delete() { for (Stream stream : streams) stream.delete(); }
	}
	private Snapshot snapshot(java.util.List<Stream> streams) throws IOException {
		java.util.List<Snapshot> parts = new java.util.ArrayList<Snapshot>();
		for (Stream stream : streams) parts.add(stream.view());
		return new Snapshot(parts);
	}
	private static final class CountedOutput extends FilterOutputStream {
		long bytes;
		CountedOutput(OutputStream out) { super(out); }
		@Override public void write(int b) throws IOException { out.write(b); bytes++; }
		@Override public void write(byte[] b, int off, int len) throws IOException { out.write(b, off, len); bytes += len; }
	}

	/** A compressed packet stream and a small self track, owned exclusively by the writer thread. */
	private final class Stream {
		final File packetFile;
		final File selfFile;
		final File audioFile;
		final Deflater audioDeflater = new Deflater(3, true);
		final DataOutputStream audio;
		final CountedOutput audioCounter;
		long audioCount;
		final Deflater deflater = new Deflater(3, true);
		final DataOutputStream packets;
		final DataOutputStream self;
		long packetBytes;
		long selfBytes;
		private int readers;
		private boolean retired;
		Snapshot view() throws IOException { flush(); return new Snapshot(packetFile, selfFile, audioFile, packetBytes, selfBytes, audioCount, audioCounter.bytes); }
		synchronized void retain() { readers++; }
		synchronized void release() { readers--; removeRetired(); }
		private void removeRetired() { if (retired && readers == 0) { packetFile.delete(); selfFile.delete(); audioFile.delete(); } }
		Stream() throws IOException {
			tempDir.mkdirs();
			String id = UUID.randomUUID().toString();
			packetFile = new File(tempDir, id + ".tmcpr");
			selfFile = new File(tempDir, id + ".self");
			audioFile = new File(tempDir, id + ".audio");
			audioCounter = new CountedOutput(new BufferedOutputStream(new DeflaterOutputStream(new BufferedOutputStream(new FileOutputStream(audioFile), BUFFER / 8), audioDeflater, BUFFER / 8, true), BUFFER / 8));
			audio = new DataOutputStream(audioCounter);
			// Batch small entity updates before entering zlib; crowded worlds send thousands per tick.
			packets = new DataOutputStream(new BufferedOutputStream(new DeflaterOutputStream(new BufferedOutputStream(new FileOutputStream(packetFile), BUFFER), deflater, BUFFER, true), BUFFER));
			self = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(selfFile), BUFFER / 8));
		}
		void packet(int t, byte[] data) throws IOException { packets.writeInt(t); packets.writeInt(data.length); packets.write(data); packetBytes += 8 + data.length; }
		void sample(SelfSample s) throws IOException { s.write(self); selfBytes += SelfSample.BYTES; }
		void audio(AudioEvent event) throws IOException { event.write(audio); audioCount++; }
		void flush() throws IOException { packets.flush(); self.flush(); audio.flush(); }
		long diskBytes() { return packetFile.length() + selfFile.length() + audioFile.length(); }
		synchronized void delete() {
			if (retired) return;
			retired = true;
			try { packets.close(); } catch (IOException ignored) {} finally { deflater.end(); }
			try { self.close(); } catch (IOException ignored) {}
			try { audio.close(); } catch (IOException ignored) {} finally { audioDeflater.end(); }
			removeRetired();
		}
	}

	/** Frozen lengths allow saving while recording continues to append to the same file. */
	private static final class Snapshot {
		final File packetFile, selfFile, audioFile;
		final long packetBytes, selfBytes, audioCount, audioBytes;
		final java.util.List<Snapshot> parts;
		Snapshot(File packetFile, File selfFile, File audioFile, long packetBytes, long selfBytes, long audioCount, long audioBytes) {
			this.packetFile = packetFile; this.selfFile = selfFile; this.audioFile = audioFile; this.audioCount = audioCount; this.packetBytes = packetBytes; this.selfBytes = selfBytes;
			this.audioBytes = audioBytes; parts = null;
		}
		Snapshot(java.util.List<Snapshot> parts) {
			this.parts = parts; packetFile = selfFile = audioFile = null;
			long p = 0, s = 0, a = 0, ab = 0;
			for (Snapshot part : parts) { p += part.packetBytes; s += part.selfBytes; a += part.audioCount; ab += part.audioBytes; }
			packetBytes = p; selfBytes = s; audioCount = a; audioBytes = ab;
		}
		DataInputStream packets() throws IOException { return reader(0); }
		DataInputStream self() throws IOException { return reader(1); }
		DataInputStream audio() throws IOException { return reader(2); }
		private DataInputStream reader(int track) throws IOException {
			if (parts != null) {
				java.util.List<InputStream> inputs = new java.util.ArrayList<InputStream>();
				try {
					for (Snapshot part : parts) inputs.add(part.reader(track));
					return new DataInputStream(new SequenceInputStream(java.util.Collections.enumeration(inputs)));
				} catch (IOException e) { for (InputStream input : inputs) try { input.close(); } catch (IOException ignored) {} throw e; }
			}
			InputStream input = track == 1 ? new BufferedInputStream(new FileInputStream(selfFile)) : packetReader(track == 0 ? packetFile : audioFile);
			final long length = track == 0 ? packetBytes : track == 1 ? selfBytes : audioBytes;
			return new DataInputStream(new FilterInputStream(input) {
				long left = length;
				@Override public int read() throws IOException { if (left == 0) return -1; int value = in.read(); if (value < 0) throw new EOFException(); left--; return value; }
				@Override public int read(byte[] b, int off, int len) throws IOException {
					if (len == 0) return 0;
					if (left == 0) return -1;
					int n = in.read(b, off, (int) Math.min(left, len)); if (n < 0) throw new EOFException(); left -= n; return n;
				}
			});
		}
	}
	private static void copyLength(InputStream in, OutputStream out, long length) throws IOException {
		byte[] buf = new byte[BUFFER];
		while (length > 0) { int n = in.read(buf, 0, (int) Math.min(buf.length, length)); if (n < 0) throw new EOFException(); out.write(buf, 0, n); length -= n; }
	}

	/** Keep a bootstrap checkpoint before from and only the requested interval after it. */
	private static void copyRange(Snapshot source, DataOutputStream packets, DataOutputStream self, int from, int to, int base,
			ReplayBackend.Classifier classifier, Stream counts) throws IOException {
		// Only the older bootstrap needs a seek plan. The recent interval is copied in full.
		Index index = Index.before(source, classifier == null ? (p, k) -> Category.ALWAYS : classifier, from);
		int cut = index.count;
		int[] earlier = SeekPlan.plan(index.category, index.key, 0, cut);
		try (DataInputStream in = source.packets()) {
			byte[] buf = new byte[1 << 16];
			int e = 0;
			long read = 0;
			for (int i = 0; read < source.packetBytes; i++) {
				int t = in.readInt(); int len = in.readInt();
				if (len < 0 || len > source.packetBytes - read - 8) throw new IOException("damaged recording");
				if (t > to) break;
				if (buf.length < len) buf = new byte[len];
				in.readFully(buf, 0, len);
				read += 8 + len;
				boolean bootstrap = i < cut && e < earlier.length && earlier[e] == i;
				if (bootstrap) e++;
				if (t <= to && (i >= cut || bootstrap)) {
					packets.writeInt(Math.max(from, t) - base); packets.writeInt(len); packets.write(buf, 0, len);
					if (counts != null) counts.packetBytes += 8 + len;
				}
			}
		}
		if (self != null) copySelf(source, self, from, to, base, counts);
		if (counts != null) copyAudio(source, counts.audio, from, to, base, counts);
	}

	private static void copyAudio(Snapshot source, DataOutputStream out, int from, int to, int base, Stream counts) throws IOException {
		try (DataInputStream in = source.audio()) {
			AudioEvent.copy(in, source.audioCount, out, from, to, base, counts == null ? null : n -> counts.audioCount += n);
		}
	}

	private static void copySelf(Snapshot source, DataOutputStream out, int from, int to, int base, Stream counts) throws IOException {
		try (DataInputStream in = source.self()) {
			SelfSample before = null;
			for (long read = 0; read < source.selfBytes; read += SelfSample.BYTES) {
				SelfSample s = SelfSample.read(in);
				if (s.time < from) { before = s; continue; }
				if (before != null) { before.time = from - base; before.write(out); if (counts != null) counts.selfBytes += SelfSample.BYTES; before = null; }
				if (s.time > to) break;
				s.time -= base; s.write(out); if (counts != null) counts.selfBytes += SelfSample.BYTES;
			}
			if (before != null) { before.time = from - base; before.write(out); if (counts != null) counts.selfBytes += SelfSample.BYTES; }
		}
	}
	private static void entry(ZipOutputStream zip, String name, byte[] data) throws IOException {
		zip.putNextEntry(new ZipEntry(name));
		zip.write(data);
		zip.closeEntry();
	}

	/** The packets written to {@code file} so far, uncompressed. */
	static DataInputStream packetReader(File file) throws IOException {
		return new DataInputStream(new InflaterInputStream(new java.io.BufferedInputStream(new FileInputStream(file), BUFFER), new Inflater(true), BUFFER) {
			@Override public void close() throws IOException {
				// A supplied Inflater is not released by InflaterInputStream.close().
				try { super.close(); } finally { inf.end(); }
			}
		});
	}

	/** The first {@code length} bytes of {@code from} (its file may still be growing); closes it. */
	private static void copy(ZipOutputStream zip, String name, InputStream from, long length) throws IOException {
		zip.putNextEntry(new ZipEntry(name));
		byte[] buf = new byte[BUFFER];
		long left = length;
		try (InputStream in = from) {
			while (left > 0) {
				int n = in.read(buf, 0, (int) Math.min(buf.length, left));
				if (n < 0) {
					break;
				}
				zip.write(buf, 0, n);
				left -= n;
			}
		}
		zip.closeEntry();
	}

	static void copyAll(InputStream in, OutputStream out) throws IOException {
		byte[] buf = new byte[BUFFER];
		int n;
		while ((n = in.read(buf)) >= 0) {
			out.write(buf, 0, n);
		}
	}

	/** Classify only packets older than a clip, to reconstruct its starting world. */
	static final class Index {
		int count;
		byte[] category = new byte[1 << 12];
		long[] key = new long[1 << 12];

		static Index before(Snapshot source, ReplayBackend.Classifier classifier, int before) throws IOException {
			Index index = new Index();
			long[] out = new long[1];
			long read = 0;
			try (DataInputStream in = source.packets()) {
				byte[] buf = new byte[1 << 16];
				while (read < source.packetBytes) {
					int t = in.readInt();
					if (t >= before) break;
					int len = in.readInt();
					if (len < 0 || len > source.packetBytes - read - 8) {
						throw new IOException("damaged recording");
					}
					if (buf.length < len) {
						buf = new byte[len];
					}
					in.readFully(buf, 0, len);
					read += 8 + len;
					out[0] = 0;
					index.add(classifier.classify(java.nio.ByteBuffer.wrap(buf, 0, len).slice(), out), out[0]);
				}
			}
			return index;
		}

		private void add(byte c, long k) {
			if (count == category.length) {
				category = java.util.Arrays.copyOf(category, count * 2);
				key = java.util.Arrays.copyOf(key, count * 2);
			}
			category[count] = c;
			key[count] = k;
			count++;
		}

	}
}
