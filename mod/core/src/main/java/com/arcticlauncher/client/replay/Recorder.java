package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.LocalClock;

import com.google.gson.Gson;
import java.io.*;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.OverlappingFileLockException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.Date;
import java.util.UUID;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.function.Consumer;
import java.util.function.IntSupplier;
import java.util.function.Supplier;
import java.util.zip.*;

import com.arcticlauncher.client.Lang;
/** A rolling replay buffer and an independent, explicitly started recording. Packet writes are serialized; immutable snapshots are saved separately. */
public final class Recorder {
	static final String PACKETS_ENTRY = "recording.tmcpr";
	static final String META_ENTRY = "metaData.json";
	static final String SELF_ENTRY = "arctic/self.bin";
	static final String EXTRAS_ENTRY = "arctic.json";
	static final String THUMB_ENTRY = "arctic/thumb.png";
	private static final int BUFFER = 1 << 18;
	private static final long MAX_PENDING_BYTES = 64L << 20;
	/** Longest a bulk producer waits for the writer before queueing anyway. */
	private static final long BULK_WAIT_NANOS = 10_000_000_000L;
	/** These limits count compressed bytes on disk, not inflated network traffic. */
	static final long MAX_RECORDING_BYTES = 512L << 20;
	static final long MIN_FREE_BYTES = 512L << 20;
	private static final int COMPACT_INTERVAL_MS = 10_000;
	private static final int CHECKPOINT_INTERVAL_MS = 1_000;
	private static final Gson GSON = new Gson();
	private final File tempDir;
	private final File savedDir;
	private final File recoveryDir;
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
	private final java.util.concurrent.atomic.AtomicReference<Take> current = new java.util.concurrent.atomic.AtomicReference<Take>();
	private final java.util.concurrent.atomic.AtomicInteger pendingSaves = new java.util.concurrent.atomic.AtomicInteger();
	private final java.util.concurrent.ExecutorService saves = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-replay-save"); t.setDaemon(true); t.setPriority(Thread.MIN_PRIORITY); return t;
	});

	public Recorder(File gameDir, IntSupplier clipSeconds, Consumer<String> log, Supplier<ReplayBackend.Classifier> sorter) {
		this(gameDir, clipSeconds, log, sorter, MAX_RECORDING_BYTES, MIN_FREE_BYTES, 0);
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
		this.recoveryDir = new File(gameDir, ".arctic/replay-recovery");
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
					log.accept("replay writer: " + t); // i18n-ignore
				}
			}
		}, "arctic-replay-writer");
		writer.setDaemon(true);
		writer.setPriority(Thread.MIN_PRIORITY + 1);
		writer.start();
		saves.execute(this::recoverRecordings);
	}

	public File savedDir() { return savedDir; }
	File tempDir() { return tempDir; }
	void afterPackets(Runnable r) { tasks.add(r); }
	void afterWrites(Runnable r) { tasks.add(() -> compactions.execute(() -> tasks.add(() -> saves.execute(r)))); }
	public enum TakeState { OFF, STATE_ONLY, BUFFERING, RECORDING, STOPPED }

	public Take current() { return current.get(); }
	public TakeState state() { Take t = current(); return t == null ? TakeState.OFF : t.state; }
	public String stopReason() { Take t = current(); return t == null ? null : t.stopReason; }
	public void enabled(boolean enabled) { Take t = current(); if (t != null) t.enabled(enabled); }
	public boolean recording() { Take t = current(); return t != null && t.state == TakeState.RECORDING; }
	public boolean bufferReady() { TakeState state = state(); return state == TakeState.BUFFERING || state == TakeState.RECORDING; }
	/** Files being written or waiting for the save worker, including manual recordings. */
	public int pendingSaves() { return pendingSaves.get(); }
	public long recordingBytes() { Take t = current(); return t == null ? 0 : t.manualDiskBytes; }
	public int recordingMillis() { Take t = current(); return t != null && t.state == TakeState.RECORDING ? Math.max(0, t.time() - t.manualFrom) : 0; }

	/** Always retain the login/bootstrap, so F9 and F10 can be used without reconnecting. */
	public synchronized Take begin(String server, boolean singleplayer, int protocol, String mcVersion) {
		Take previous = current();
		if (previous != null) previous.end();
		boolean enabled = com.arcticlauncher.client.ArcticClient.config() == null
				|| com.arcticlauncher.client.ArcticClient.config().replayEnabled;
		Take take = new Take(server, singleplayer, protocol, mcVersion, enabled);
		tasks.add(() -> {
			try { take.ring = new Ring(); } catch (IOException e) { take.fail(e.toString()); }
		});
		current.set(take);
		return take;
	}

	public boolean saveMoment(Consumer<File> done) { return saveMoment(null, done); }
	public boolean saveMoment(byte[] thumbnail, Consumer<File> done) {
		Take t = current();
		return t != null && t.clip(thumbnail, done);
	}

	/** The callback receives the completed manual recording, including when leaving the world. */
	public boolean startRecording(Consumer<File> done) {
		Take t = current();
		return t != null && t.startRecording(done);
	}
	public boolean mark() {
		Take t = current();
		return t != null && t.mark();
	}

	public boolean stopRecording() {
		Take t = current();
		return t != null && t.stopRecording();
	}

	public final class Take {
		private volatile String server;
		final boolean singleplayer;
		final int protocol;
		final String mcVersion;
		private final long startNanos = nanoTime.getAsLong();
		private long pausedNanos;
		private long pausedAt = -1;
		private volatile boolean ended;
		private volatile TakeState state;
		private volatile String stopReason;
		/** Writer-side copy of the rolling-window setting, ordered with incoming packets. */
		private boolean buffering;
		private int windowStart;
		private int enabledAt;
		private volatile int manualFrom;
		private long recordingRequest;
		private volatile int latest;
		private volatile int selfId = -1;
		private volatile String selfName;
		private volatile UUID selfUuid;
		// Writer thread only. Ring and manual never share writable files.
		private Ring ring;
		private boolean compacting;
		private Stream manual;
		private Journal journal;
		private int checkpointAt;
		private final java.util.List<Integer> manualMarkers = new java.util.ArrayList<Integer>();
		private Consumer<File> manualDone;
		private int manualBase;
		private long manualDate;
		private int compactedAt;
		private int checkedAt = -1;
		private volatile long manualDiskBytes;
		private long pending;

		Take(String server, boolean singleplayer, int protocol, String mcVersion, boolean enabled) {
			buffering = enabled;
			state = enabled ? TakeState.BUFFERING : TakeState.STATE_ONLY;
			this.server = server == null ? Lang.t("replay.unknown") : server;
			this.singleplayer = singleplayer;
			this.protocol = protocol;
			this.mcVersion = mcVersion;
		}
		private synchronized void enabled(boolean enabled) {
			if (ended || state == TakeState.STOPPED || enabled == (state != TakeState.STATE_ONLY)) return;
			int at = time();
			enabledAt = at;
			state = enabled ? TakeState.BUFFERING : TakeState.STATE_ONLY;
			tasks.add(() -> {
				buffering = enabled;
				windowStart = at;
				// Turning Replay off discards the unsaved manual take.
				if (!enabled && manual != null) {
					manual.delete();
					manual = null;
					manualDone = null;
					journal.close(true);
					journal = null;
				}
			});
		}

		public synchronized boolean accepting(String packet) {
			return !ended && state != TakeState.STOPPED
					&& (state != TakeState.STATE_ONLY || PacketSorter.category(packet) != Category.TRANSIENT);
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
			if (ended || state == TakeState.STOPPED || data == null) return;
			if (pending + data.length > MAX_PENDING_BYTES) {
				stop(Lang.t("replay.replay_data_arrived_faster_than_it_could_be"));
				return;
			}
			final int t = time();
			pending += data.length;
			tasks.add(() -> {
				synchronized (Take.this) { pending -= data.length; Take.this.notifyAll(); }
				if (ring == null || state == TakeState.STOPPED) return;
				try {
					ring.active().packet(t, data);
					latest = t;
					if (manual != null) manual.packet(Math.max(0, t - manualBase), data);
					maintain(t);
				} catch (IOException | RuntimeException e) { fail(e.toString()); }
			});
		}

		/**
		 * A packet from a producer that can wait (a snapshot of the whole world, written in one go):
		 * waits for the writer to catch up rather than running into the pending limit, which is meant
		 * for a network that outruns the disk and would end the buffer. Not for the network thread.
		 */
		public void packetWhenRoom(final byte[] data) {
			if (data == null) return;
			synchronized (this) {
				long deadline = System.nanoTime() + BULK_WAIT_NANOS;
				while (!ended && state != TakeState.STOPPED && pending > 0 && pending + data.length > MAX_PENDING_BYTES / 2) {
					long left = deadline - System.nanoTime();
					if (left <= 0) break;
					try {
						wait(Math.max(1, left / 1_000_000L));
					} catch (InterruptedException e) {
						Thread.currentThread().interrupt();
						break;
					}
				}
			}
			packet(data);
		}

		public synchronized void sample(final SelfSample sample) {
			if (ended || state == TakeState.STOPPED) return;
			sample.time = time();
			tasks.add(() -> {
				if (ring == null || state == TakeState.STOPPED) return;
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
			if (ended || state == TakeState.STOPPED) return;
			tasks.add(() -> {
				if (ring == null || state == TakeState.STOPPED) return;
				if (!buffering) return;
				try {
					ring.active().audio(event);
					if (manual != null) manual.audio(event.at(event.time, manualBase));
				} catch (IOException e) { fail(e.toString()); }
			});
		}

		private void maintain(int t) throws IOException {
			if (manual != null && t - checkpointAt >= CHECKPOINT_INTERVAL_MS) checkpoint(t);
			int window = buffering ? clipSeconds.getAsInt() * 1000 : 0;
			int from = Math.max(windowStart, t - window);
			int interval = compactInterval > 0 ? compactInterval : buffering ? window : COMPACT_INTERVAL_MS;
			if (!compacting && t - compactedAt >= interval && from > 0) {
				ReplayBackend.Classifier classifier = sorter.get();
				if (classifier != null) {
					final Ring buffer = ring;
					final Snapshot source = buffer.view();
					final java.util.List<Stream> prefix = new java.util.ArrayList<Stream>(buffer.streams);
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
							tasks.add(() -> { compacting = false; if (!ended && state != TakeState.STOPPED) fail("buffer cleanup failed: " + e); }); // i18n-ignore
						} finally { for (Stream stream : prefix) stream.release(); }
					});
				}
				compactedAt = t;
			}
			if (checkedAt < 0 || t - checkedAt >= 5_000) {
				checkedAt = t;
				ring.flush();
				if (tempDir.getUsableSpace() < minFreeBytes) fail(Lang.t("replay.less_than_512_mb_of_disk_space_remains"));
				else if (ring.diskBytes() > maxBytes) fail(Lang.t("replay.the_replays_world_data_is_too_large_for_the"));
			}
		}

		private synchronized boolean clip(byte[] thumbnail, Consumer<File> done) {
			if (ended || state == TakeState.STATE_ONLY) return false;
			final int at = state == TakeState.STOPPED ? latest : time();
			final int from = Math.max(enabledAt, at - clipSeconds.getAsInt() * 1000);
			tasks.add(() -> saveLater(ring, from, at, Lang.t("replay.clip"), thumbnail, done));
			return true;
		}
		private synchronized boolean startRecording(Consumer<File> done) {
			if (ended || state != TakeState.BUFFERING) return false;
			final int at = time();
			final long request = ++recordingRequest;
			state = TakeState.RECORDING;
			manualFrom = at;
			tasks.add(() -> {
				manualBase = at;
				manualDate = System.currentTimeMillis();
				manualDiskBytes = 0;
				manualMarkers.clear();
				manualDone = done;
				try {
					journal = new Journal(recoveryDir, archiveName(Lang.t("replay.recording")));
					manual = new Stream(journal.dir, "take");
					// Seed a fresh stream with the world at this instant, then append live packets.
					copyRange(ring.view(), manual.packets, manual.self, at, at, at, sorter.get(), manual);
					checkpoint(at);
				} catch (IOException | RuntimeException e) {
					if (manual != null) manual.delete();
					manual = null;
					if (journal != null) journal.close(true);
					journal = null;
					manualDone = null;
					synchronized (Take.this) {
						if (recordingRequest == request && state == TakeState.RECORDING) state = TakeState.BUFFERING;
					}
					log.accept("replay: recording start failed: " + e); // i18n-ignore
					if (done != null) done.accept(null);
				}
			});
			return true;
		}
		private synchronized boolean mark() {
			if (state != TakeState.RECORDING || ended) return false;
			int mark = time() - manualFrom;
			tasks.add(() -> { if (manual != null) manualMarkers.add(mark); });
			return true;
		}

		private synchronized boolean stopRecording() {
			if (ended || state != TakeState.RECORDING) return false;
			state = TakeState.BUFFERING;
			final int at = time();
			tasks.add(() -> finishManual(at));
			return true;
		}
		private void finishManual(int at) {
			Stream stream = manual;
			Journal owner = journal;
			Consumer<File> done = manualDone;
			manual = null;
			journal = null;
			manualDone = null;
			if (stream == null) return;
			try {
				Snapshot view = stream.view();
				Checkpoint cp = recordingCheckpoint(view, at);
				owner.write(cp);
				stream.close();
				pendingSaves.incrementAndGet();
				saves.execute(() -> {
					File file = null;
					try {
						if (view.packetBytes != 0) file = writeArchive(view, 0, cp.meta, cp.extras, null, owner.output(savedDir));
					} catch (IOException | RuntimeException e) {
						log.accept("replay: save failed: " + e); // i18n-ignore
					} finally {
						owner.close(file != null || view.packetBytes == 0);
						pendingSaves.decrementAndGet();
					}
					if (done != null) done.accept(file);
				});
			} catch (IOException e) {
				stream.close();
				owner.close(false); // The last complete checkpoint is still recoverable.
				log.accept("replay: recording checkpoint failed: " + e); // i18n-ignore
				if (done != null) done.accept(null);
			}
		}

		private void checkpoint(int at) throws IOException {
			Snapshot view = manual.view();
			journal.write(recordingCheckpoint(view, at));
			manualDiskBytes = manual.diskBytes();
			checkpointAt = at;
		}

		private Checkpoint recordingCheckpoint(Snapshot view, int at) {
			int duration = Math.max(0, at - manualBase);
			return new Checkpoint(metadata(duration, manualDate), extras(duration, new java.util.ArrayList<Integer>(manualMarkers)), view);
		}
		public synchronized void end() {
			if (ended) return;
			ended = true;
			final int at = time();
			current.compareAndSet(this, null);
			tasks.add(() -> {
				if (manual != null) finishManual(at);
				if (ring != null) ring.delete();
				ring = null;
			});
		}
		public synchronized void stop(String why) {
			if (ended || state == TakeState.STOPPED) return;
			stopReason = why;
			state = TakeState.STOPPED;
			tasks.add(() -> fail(why));
		}

		private void fail(String why) {
			stopReason = why;
			state = TakeState.STOPPED;
			log.accept("replay: stopped: " + why); // i18n-ignore
			notice(Lang.t("replay.replay_buffer_stopped"), why + Lang.t("replay.saved_clips_are_safe"));
			if (manual != null) finishManual(time());
		}
		private void saveLater(Ring buffer, int from, int to, String kind, byte[] thumbnail, Consumer<File> done) {
			saveLater(buffer == null ? java.util.Collections.emptyList() : new java.util.ArrayList<Stream>(buffer.streams), from, to, kind, thumbnail, done, java.util.Collections.emptyList());
		}
		private void saveLater(java.util.List<Stream> streams, int from, int to, String kind, byte[] thumbnail, Consumer<File> done,
				java.util.List<Integer> markers) {
			if (streams.isEmpty()) { if (done != null) done.accept(null); return; }
			try {
				Snapshot view = snapshot(streams);
				for (Stream stream : streams) stream.retain();
				pendingSaves.incrementAndGet();
				saves.execute(() -> {
					try { File file = save(view, from, to, kind, thumbnail, markers); if (done != null) done.accept(file); }
					finally {
						pendingSaves.decrementAndGet();
						for (Stream stream : streams) stream.release();
					}
				});
			} catch (IOException e) { log.accept("replay: snapshot failed: " + e); if (done != null) done.accept(null); } // i18n-ignore
		}
		private File save(Snapshot stream, int from, int to, String kind, byte[] thumbnail, java.util.List<Integer> markers) {
			if (stream == null || stream.packetBytes == 0) return null;
			try {
				return writeArchive(stream, from, metadata(to - from, System.currentTimeMillis()), extras(to - from, markers), thumbnail,
						new File(savedDir, archiveName(kind)));
			} catch (IOException | RuntimeException e) {
				log.accept("replay: save failed: " + e); return null; // i18n-ignore
			}
		}

		private String archiveName(String kind) {
			String where = server.replaceAll("[^A-Za-z0-9 ._-]", "_").trim();
			if (where.length() > 40) where = where.substring(0, 40).trim();
			String when = LocalClock.formatter("yyyy-MM-dd HH-mm-ss-SSS").format(new Date());
			return (where.isEmpty() ? "Replay" : where) + " " + when + " " + kind.replaceAll("[\\\\/:*?\"<>|]", "_")
					+ " " + UUID.randomUUID().toString().substring(0, 8) + ".mcpr";
		}

		private ReplayMeta metadata(int duration, long recordedAt) {
			ReplayMeta meta = new ReplayMeta();
			meta.singleplayer = singleplayer; meta.serverName = server; meta.duration = duration;
			meta.date = recordedAt; meta.protocol = protocol; meta.selfId = selfId; meta.mcversion = mcVersion;
			return meta;
		}

		private ReplayMeta.Extras extras(int duration, java.util.List<Integer> markers) {
			ReplayMeta.Extras extras = new ReplayMeta.Extras();
			extras.selfName = selfName; extras.selfUuid = selfUuid == null ? null : selfUuid.toString();
			extras.clipSeconds = clipSeconds.getAsInt(); extras.moments.add(duration);
			extras.markers = markers;
			return extras;
		}
	}

	private File writeArchive(Snapshot stream, int from, ReplayMeta meta, ReplayMeta.Extras extras, byte[] thumbnail, File out) throws IOException {
		int to = from + meta.duration;
		File part = new File(out.getPath() + ".part");
		Files.createDirectories(savedDir.toPath());
		try {
			try (ZipOutputStream zip = new ZipOutputStream(new BufferedOutputStream(new FileOutputStream(part), BUFFER))) {
				zip.setLevel(6);
				// Batch track writes before calling zlib.
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
			Files.move(part.toPath(), out.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
			return out;
		} finally {
			Files.deleteIfExists(part.toPath());
		}
	}

	private void recoverRecordings() {
		File[] dirs = recoveryDir.listFiles(File::isDirectory);
		if (dirs == null) return;
		for (File dir : dirs) {
			// Only the process that owns the lock can read or discard a take.
			File lockFile = new File(dir, "owner");
			// The creator takes the lock before creating any track files.
			if (!lockFile.isFile() || !new File(dir, "take.audio").isFile()) continue;
			boolean recovered = false;
			try (FileChannel lock = FileChannel.open(lockFile.toPath(), StandardOpenOption.WRITE)) {
				try (FileLock claim = lock.tryLock()) {
					if (claim == null) continue;
					File file = new File(dir, "checkpoint.json");
					if (file.isFile()) {
						Checkpoint cp;
						try (Reader in = Files.newBufferedReader(file.toPath(), StandardCharsets.UTF_8)) {
							cp = GSON.fromJson(in, Checkpoint.class);
						}
						if (cp == null || cp.meta == null || cp.extras == null || cp.output == null || !cp.output.endsWith(".mcpr")
								|| !cp.output.equals(new File(cp.output).getName())) throw new IOException("Invalid replay checkpoint");
						File out = new File(savedDir, cp.output);
						if (!out.isFile() && cp.packetBytes > 0) {
							writeArchive(cp.snapshot(dir), 0, cp.meta, cp.extras, null, out);
							log.accept("replay: recovered " + out.getName()); // i18n-ignore
						}
					}
					Journal.clear(dir);
					recovered = true;
				}
			} catch (OverlappingFileLockException ignored) {
				// Another recorder in this process owns the take.
			} catch (IOException | RuntimeException e) {
				log.accept("replay: recovery could not finish " + dir.getName() + ": " + e); // i18n-ignore
			}
			if (recovered) {
				lockFile.delete();
				dir.delete();
			}
		}
	}

	/** Lengths published only after all tracks are flushed, so a partial tail is never read. */
	private static final class Checkpoint {
		final ReplayMeta meta;
		final ReplayMeta.Extras extras;
		final long packetBytes, selfBytes, audioCount, audioBytes;
		String output;

		Checkpoint(ReplayMeta meta, ReplayMeta.Extras extras, Snapshot view) {
			this.meta = meta;
			this.extras = extras;
			packetBytes = view.packetBytes;
			selfBytes = view.selfBytes;
			audioCount = view.audioCount;
			audioBytes = view.audioBytes;
		}

		Snapshot snapshot(File dir) {
			return new Snapshot(new File(dir, "take.tmcpr"), new File(dir, "take.self"), new File(dir, "take.audio"),
					packetBytes, selfBytes, audioCount, audioBytes);
		}
	}

	private static final class Journal {
		final File dir;
		private final String output;
		private final FileChannel lock;

		Journal(File parent, String output) throws IOException {
			this.output = output;
			Files.createDirectories(parent.toPath());
			dir = new File(parent, UUID.randomUUID().toString());
			Files.createDirectory(dir.toPath());
			lock = FileChannel.open(new File(dir, "owner").toPath(), StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
			try { lock.lock(); } catch (IOException | RuntimeException e) { lock.close(); throw e; }
		}

		File output(File parent) { return new File(parent, output); }

		void write(Checkpoint cp) throws IOException {
			cp.output = output;
			File next = new File(dir, "checkpoint.next");
			Files.write(next.toPath(), GSON.toJson(cp).getBytes(StandardCharsets.UTF_8));
			Files.move(next.toPath(), new File(dir, "checkpoint.json").toPath(), StandardCopyOption.ATOMIC_MOVE,
					StandardCopyOption.REPLACE_EXISTING);
		}

		void close(boolean discard) {
			if (discard) clear(dir);
			try { lock.close(); } catch (IOException ignored) {}
			if (discard) {
				new File(dir, "owner").delete();
				dir.delete();
			}
		}

		static void clear(File dir) {
			for (String name : new String[] {"checkpoint.json", "checkpoint.next", "take.tmcpr", "take.self", "take.audio"}) {
				new File(dir, name).delete();
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
		private boolean closed;
		Snapshot view() throws IOException { flush(); return new Snapshot(packetFile, selfFile, audioFile, packetBytes, selfBytes, audioCount, audioCounter.bytes); }
		synchronized void retain() { readers++; }
		synchronized void release() { readers--; removeRetired(); }
		private void removeRetired() { if (retired && readers == 0) { packetFile.delete(); selfFile.delete(); audioFile.delete(); } }
		Stream() throws IOException {
			this(tempDir, UUID.randomUUID().toString());
		}
		Stream(File dir, String id) throws IOException {
			Files.createDirectories(dir.toPath());
			packetFile = new File(dir, id + ".tmcpr");
			selfFile = new File(dir, id + ".self");
			audioFile = new File(dir, id + ".audio");
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
			close();
			removeRetired();
		}
		void close() {
			if (closed) return;
			closed = true;
			try { packets.close(); } catch (IOException ignored) {} finally { deflater.end(); }
			try { self.close(); } catch (IOException ignored) {}
			try { audio.close(); } catch (IOException ignored) {} finally { audioDeflater.end(); }
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
		int[] earlier = SeekPlan.plan(index.category, index.key, 0, cut, index.removals, index.bulks);
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
		final java.util.Map<Integer, int[]> removals = new java.util.HashMap<Integer, int[]>();
		final java.util.Map<Integer, long[]> bulks = new java.util.HashMap<Integer, long[]>();

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
					java.nio.ByteBuffer packet = java.nio.ByteBuffer.wrap(buf, 0, len).slice();
					byte category = classifier.classify(packet, out);
					if (category == Category.ENTITY_REMOVE) {
						int[] ids = classifier.removedEntities(packet);
						if (ids != null) index.removals.put(index.count, ids);
					} else if (category == Category.CHUNK_BULK) {
						long[] chunks = classifier.bulkChunks(packet);
						if (chunks != null) index.bulks.put(index.count, chunks);
					}
					index.add(category, out[0]);
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
