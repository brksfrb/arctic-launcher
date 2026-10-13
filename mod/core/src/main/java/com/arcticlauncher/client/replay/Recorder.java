package com.arcticlauncher.client.replay;

import com.google.gson.Gson;
import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.function.Consumer;
import java.util.zip.Deflater;
import java.util.zip.DeflaterOutputStream;
import java.util.zip.Inflater;
import java.util.zip.InflaterInputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Records every session quietly: packets as the game received them go to a
 * temp file on a background thread (the game thread only hands them over),
 * compressed as they're written. Nothing is kept unless the player saves; then
 * the session becomes a replay in {@code replay_recordings} (updated again when
 * they leave).
 *
 * <p>A long stay at a busy server is compacted instead of growing without end:
 * once the packets pass {@link #COMPACT_AT}, everything older than the last
 * {@link #KEEP_MS} is cut down to what it takes to show the world as it was then
 * ({@link SeekPlan}), so recording goes on in bounded space.
 */
public final class Recorder {
	static final String PACKETS_ENTRY = "recording.tmcpr";
	static final String META_ENTRY = "metaData.json";
	static final String SELF_ENTRY = "arctic/self.bin";
	static final String EXTRAS_ENTRY = "arctic.json";
	static final String THUMB_ENTRY = "arctic/thumb.png";
	private static final int BUFFER = 1 << 18;
	/** Queued but unwritten data past this is dropped (the disk can't keep up). */
	private static final long MAX_PENDING_BYTES = 256L << 20;
	/**
	 * A session stops being recorded once its packets reach this much: a long
	 * stay at a crowded server would otherwise fill the whole drive (an hour
	 * with a thousand players around is tens of gigabytes).
	 */
	static final long MAX_RECORDING_BYTES = 1L << 30;
	/** ...and the self-track (one small sample per tick) at this much. */
	private static final long MAX_SELF_BYTES = 64L << 20;
	/** Packets (uncompressed) past which older parts of the session are compacted. */
	static final long COMPACT_AT = 512L << 20;
	/** How much of the session compaction keeps whole, at least (and twice the clip length). */
	static final int KEEP_MS = 10 * 60 * 1000;
	/** Compaction that frees less than this share isn't worth repeating: the size limit applies. */
	private static final double MIN_COMPACT_GAIN = 0.25;
	/** Recording also stops when the drive has less than this left. */
	static final long MIN_FREE_BYTES = 10L << 30;
	/** How much is written between looks at the drive's free space. */
	private static final long FREE_CHECK_EVERY = 64L << 20;
	private static final Gson GSON = new Gson();

	private final File tempDir;
	private final File savedDir;
	private final java.util.function.BooleanSupplier enabled;
	private final java.util.function.IntSupplier clipSeconds;
	private final Consumer<String> log;
	private final long maxBytes;
	private final long minFreeBytes;
	private final long compactAt;
	private final int keepMs;
	/** Sorts packets for compaction (null: this version can't, and the size limit applies). */
	private final java.util.function.Supplier<ReplayBackend.Classifier> sorter;
	/** The "recording stopped" notice was shown (once per game is enough). */
	private volatile boolean toldFull;
	private final LinkedBlockingQueue<Runnable> tasks = new LinkedBlockingQueue<Runnable>();
	private volatile Take current;

	/**
	 * Recordings for {@code gameDir}; {@code enabled} is the recording switch,
	 * {@code clipSeconds} how far back a moment reaches, {@code log} for problems.
	 */
	public Recorder(File gameDir, java.util.function.BooleanSupplier enabled, java.util.function.IntSupplier clipSeconds, Consumer<String> log) {
		this(gameDir, enabled, clipSeconds, log, () -> null);
	}

	/** {@code sorter} lets long sessions be compacted (see the class comment). */
	public Recorder(File gameDir, java.util.function.BooleanSupplier enabled, java.util.function.IntSupplier clipSeconds, Consumer<String> log,
			java.util.function.Supplier<ReplayBackend.Classifier> sorter) {
		this(gameDir, enabled, clipSeconds, log, MAX_RECORDING_BYTES, MIN_FREE_BYTES, COMPACT_AT, KEEP_MS, sorter);
	}

	/** With other limits (for tests): packets per session, and free space the drive must keep. */
	Recorder(File gameDir, java.util.function.BooleanSupplier enabled, java.util.function.IntSupplier clipSeconds, Consumer<String> log,
			long maxBytes, long minFreeBytes) {
		this(gameDir, enabled, clipSeconds, log, maxBytes, minFreeBytes, Long.MAX_VALUE, KEEP_MS, () -> null);
	}

	/** ...and when to compact, keeping how much (ms) whole, with what. */
	Recorder(File gameDir, java.util.function.BooleanSupplier enabled, java.util.function.IntSupplier clipSeconds, Consumer<String> log,
			long maxBytes, long minFreeBytes, long compactAt, int keepMs, java.util.function.Supplier<ReplayBackend.Classifier> sorter) {
		this.keepMs = keepMs;
		this.maxBytes = maxBytes;
		this.minFreeBytes = minFreeBytes;
		this.compactAt = compactAt;
		this.sorter = sorter;
		this.tempDir = new File(gameDir, ".arctic/replay-temp");
		this.savedDir = new File(gameDir, "replay_recordings");
		this.enabled = enabled;
		this.clipSeconds = clipSeconds;
		this.log = log;
		clearTemp();
		Thread writer = new Thread(this::writeLoop, "arctic-replay-writer");
		writer.setDaemon(true);
		writer.setPriority(Thread.NORM_PRIORITY - 1);
		writer.start();
	}

	public File savedDir() {
		return savedDir;
	}

	/** Leftovers from a crash: nobody saved them. */
	private void clearTemp() {
		File[] old = tempDir.listFiles();
		if (old != null) {
			for (File f : old) {
				f.delete();
			}
		}
	}

	private void writeLoop() {
		while (true) {
			try {
				tasks.take().run();
			} catch (InterruptedException e) {
				return;
			} catch (Throwable t) {
				log.accept("replay writer: " + t);
			}
		}
	}

	/** Run {@code r} on the writer thread once everything queued before it is written. */
	void afterWrites(Runnable r) {
		tasks.add(r);
	}

	File tempDir() {
		return tempDir;
	}

	/** The session being recorded, or null. */
	public Take current() {
		return current;
	}

	/**
	 * A connection is logging in: start recording it (null when recording is
	 * off). {@code server} is the address, or the world's name in singleplayer.
	 */
	public Take begin(String server, boolean singleplayer, int protocol, String mcVersion) {
		Take previous = current;
		if (previous != null) {
			previous.end();
		}
		if (!enabled.getAsBoolean()) {
			return null;
		}
		Take take = new Take(server, singleplayer, protocol, mcVersion);
		current = take;
		tasks.add(take::open);
		return take;
	}

	/**
	 * Keep the current session as a replay, marking this moment (the end of
	 * a 2D clip). {@code done} gets the file, or null (on a background thread).
	 */
	public boolean saveMoment(Consumer<File> done) {
		return saveMoment(null, done);
	}

	/** {@link #saveMoment(Consumer)} with a picture of the moment (a small PNG, or null). */
	public boolean saveMoment(byte[] thumbnail, Consumer<File> done) {
		Take take = current;
		if (take == null) {
			return false;
		}
		if (thumbnail != null) {
			take.thumbnail = thumbnail;
		}
		take.keep(take.time(), done);
		return true;
	}

	/** One recorded session. */
	public final class Take {
		private volatile String server;
		final boolean singleplayer;
		final int protocol;
		final String mcVersion;
		final long date = System.currentTimeMillis();
		private final long startNanos = System.nanoTime();
		private long pausedNanos;
		private long pausedAt = -1;
		/** Replaced by each compaction. */
		private File packetsFile;
		private final File selfFile;
		// Writer thread only:
		private DataOutputStream packets;
		private Deflater deflater;
		private DataOutputStream self;
		/** Uncompressed, as the replay will hold them. */
		private long packetBytes;
		/** Compaction is due again past this (it grows when compaction gains little). */
		private long compactNext;
		/** Time of the oldest packet in the file (-1: none yet). */
		private int firstTime = -1;
		private long selfBytes;
		private boolean broken;
		/** Recording stopped on purpose (size limit or a nearly full drive); what's recorded is kept as is. */
		private boolean full;
		private long checkedFreeAt;
		// Any thread:
		private final java.util.concurrent.atomic.AtomicLong pending = new java.util.concurrent.atomic.AtomicLong();
		private volatile boolean ended;
		private volatile int selfId = -1;
		private volatile String selfName;
		private volatile UUID selfUuid;
		private final List<Integer> moments = new ArrayList<Integer>();
		private volatile File keptAs;
		/** The latest kept moment's picture, for replay lists. */
		private volatile byte[] thumbnail;
		private volatile int lastTime;

		Take(String server, boolean singleplayer, int protocol, String mcVersion) {
			this.server = server == null ? "Unknown" : server;
			this.singleplayer = singleplayer;
			this.protocol = protocol;
			this.mcVersion = mcVersion;
			String id = Long.toString(date, 36) + "-" + Integer.toString((int) (Math.random() * 1e6), 36);
			this.packetsFile = new File(tempDir, id + ".tmcpr");
			this.selfFile = new File(tempDir, id + ".self");
		}

		private void open() {
			try {
				tempDir.mkdirs();
				packets = packetStream(packetsFile);
				compactNext = compactAt;
				self = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(selfFile), BUFFER / 8));
			} catch (IOException e) {
				broken = true;
				log.accept("replay: can't record: " + e);
			}
		}

		/** Milliseconds recorded so far (pauses in singleplayer don't count). */
		public synchronized int time() {
			long now = pausedAt >= 0 ? pausedAt : System.nanoTime();
			return (int) ((now - startNanos - pausedNanos) / 1_000_000L);
		}

		/** Singleplayer paused (the world stands still, so the replay does too). */
		public synchronized void paused(boolean paused) {
			if (paused && pausedAt < 0) {
				pausedAt = System.nanoTime();
			} else if (!paused && pausedAt >= 0) {
				pausedNanos += System.nanoTime() - pausedAt;
				pausedAt = -1;
			}
		}

		/** The server's address as the player knows it (replaces the one guessed at login). */
		public void name(String server) {
			if (server != null && !server.isEmpty() && !singleplayer) {
				this.server = server;
			}
		}

		/** Who's recording (their entity id changes with each login). */
		public void self(int entityId, String name, UUID uuid) {
			this.selfId = entityId;
			this.selfName = name;
			this.selfUuid = uuid;
		}

		/** A packet the game received: its id and body, encoded. Any thread. */
		public void packet(final byte[] data) {
			if (ended || data == null) {
				return;
			}
			if (pending.get() > MAX_PENDING_BYTES) {
				return;
			}
			final int t = time();
			lastTime = t;
			pending.addAndGet(data.length);
			tasks.add(() -> {
				pending.addAndGet(-data.length);
				if (packets == null || broken || full) {
					return;
				}
				try {
					packets.writeInt(t);
					packets.writeInt(data.length);
					packets.write(data);
					packetBytes += 8 + data.length;
					if (firstTime < 0) {
						firstTime = t;
					}
					checkLimits();
				} catch (IOException e) {
					broken = true;
				}
			});
		}

		/** This tick's player sample (game thread). */
		public void sample(final SelfSample s) {
			if (ended) {
				return;
			}
			s.time = time();
			tasks.add(() -> {
				if (self == null || broken || full) {
					return;
				}
				try {
					s.write(self);
					selfBytes += SelfSample.BYTES;
				} catch (IOException e) {
					broken = true;
				}
			});
		}

		/** Writer thread: packets go in compressed (sync-flushed, so a save can read all written so far). */
		private DataOutputStream packetStream(File file) throws IOException {
			deflater = new Deflater(Deflater.BEST_SPEED, true);
			return new DataOutputStream(new DeflaterOutputStream(new BufferedOutputStream(new FileOutputStream(file), BUFFER), deflater, BUFFER, true));
		}

		/** Writer thread: compact a long session; stop at the size limit, or when the drive is nearly full. */
		private void checkLimits() {
			if (packetBytes >= compactNext) {
				compact();
				if (broken) {
					return;
				}
			}
			String why = null;
			if (packetBytes >= maxBytes || selfBytes >= MAX_SELF_BYTES) {
				why = "the size limit was reached";
			} else if (packetBytes - checkedFreeAt >= FREE_CHECK_EVERY || checkedFreeAt == 0) {
				checkedFreeAt = Math.max(1, packetBytes);
				if (tempDir.getUsableSpace() < minFreeBytes) {
					why = "the drive is nearly full";
				}
			}
			if (why != null) {
				full = true;
				try {
					packets.flush();
					self.flush();
				} catch (IOException e) {
					broken = true;
				}
				log.accept("replay: recording stopped, " + why);
				if (!toldFull) {
					toldFull = true;
					com.arcticlauncher.client.notice.Notices.post("Replay recording stopped",
							"Your drive is protected: " + why + ". What was recorded can still be saved.");
				}
			}
		}

		/**
		 * Writer thread: everything older than the kept window is cut down to what
		 * shows the world as it was where the window starts. A replay saved earlier
		 * is written out first and then left as it is (the next save starts a new one).
		 */
		private void compact() {
			ReplayBackend.Classifier classifier = sorter.get();
			if (classifier == null || packets == null) {
				compactNext = Long.MAX_VALUE;
				return;
			}
			int window = Math.max(keepMs, clipSeconds.getAsInt() * 2000);
			if (firstTime >= lastTime - window) {
				// All of it is within the kept window: nothing to cut yet, which says nothing about
				// what compaction will gain later. Try again further on.
				compactNext = packetBytes + compactAt;
				return;
			}
			if (keptAs != null) {
				write();
				keptAs = null;
				synchronized (moments) {
					moments.clear();
				}
			}
			long before = packetBytes;
			File next = new File(tempDir, packetsFile.getName() + "+");
			try {
				packets.close();
				deflater.end();
				packets = null;
				Index index = Index.of(packetsFile, packetBytes, classifier);
				int cut = index.firstAt(lastTime - window);
				int[] earlier = SeekPlan.plan(index.category, index.key, 0, cut);
				int windowStart = cut < index.count ? index.time[cut] : lastTime;
				DataOutputStream out = packetStream(next);
				long written = 0;
				try (DataInputStream in = packetReader(packetsFile)) {
					byte[] buf = new byte[1 << 16];
					int e = 0;
					for (int i = 0; i < index.count; i++) {
						int t = in.readInt();
						int len = in.readInt();
						if (buf.length < len) {
							buf = new byte[len];
						}
						in.readFully(buf, 0, len);
						boolean earlierKept = i < cut && e < earlier.length && earlier[e] == i;
						if (earlierKept) {
							e++;
							// The world as it was: all of it at the window's start.
							t = windowStart;
						}
						if (i >= cut || earlierKept) {
							out.writeInt(t);
							out.writeInt(len);
							out.write(buf, 0, len);
							written += 8 + len;
						}
					}
				}
				packetsFile.delete();
				packets = out;
				packetsFile = next;
				packetBytes = written;
				firstTime = written > 0 ? windowStart : -1;
			} catch (IOException | RuntimeException e) {
				broken = true;
				log.accept("replay: compaction failed, recording stopped: " + e);
				return;
			}
			log.accept("replay: compacted " + (before >> 20) + " MB of packets to " + (packetBytes >> 20) + " MB");
			compactNext = before - packetBytes < before * MIN_COMPACT_GAIN ? Long.MAX_VALUE : packetBytes + compactAt;
		}

		/** Keep this session; mark a moment at {@code at}. */
		void keep(final int at, final Consumer<File> done) {
			synchronized (moments) {
				moments.add(at);
			}
			tasks.add(() -> {
				File out = write();
				if (done != null) {
					done.accept(out);
				}
			});
		}

		/** Leaving: write the replay if it's kept, else throw it away. */
		public void end() {
			if (ended) {
				return;
			}
			ended = true;
			if (current == this) {
				current = null;
			}
			tasks.add(() -> {
				if (keptAs != null) {
					write();
				}
				close();
				packetsFile.delete();
				selfFile.delete();
			});
		}

		private void close() {
			try {
				if (packets != null) {
					packets.close();
					deflater.end();
				}
				if (self != null) {
					self.close();
				}
			} catch (IOException ignored) {
				// Deleted next anyway.
			}
			packets = null;
			self = null;
		}

		/** Write (or rewrite) the replay file from what's recorded so far. Writer thread. */
		private File write() {
			if (broken || packets == null) {
				return null;
			}
			try {
				packets.flush();
				self.flush();
				File out = keptAs != null ? keptAs : new File(savedDir, fileName());
				savedDir.mkdirs();
				File part = new File(out.getPath() + ".part");
				try (ZipOutputStream zip = new ZipOutputStream(new BufferedOutputStream(new FileOutputStream(part), BUFFER))) {
					zip.setLevel(Deflater.BEST_SPEED);
					entry(zip, META_ENTRY, GSON.toJson(meta()).getBytes(StandardCharsets.UTF_8));
					entry(zip, EXTRAS_ENTRY, GSON.toJson(extras()).getBytes(StandardCharsets.UTF_8));
					copy(zip, PACKETS_ENTRY, packetReader(packetsFile), packetBytes);
					copy(zip, SELF_ENTRY, new FileInputStream(selfFile), selfBytes);
					byte[] thumb = thumbnail;
					if (thumb != null) {
						entry(zip, THUMB_ENTRY, thumb);
					}
				}
				if (out.exists() && !out.delete()) {
					part.delete();
					return null;
				}
				if (!part.renameTo(out)) {
					return null;
				}
				keptAs = out;
				return out;
			} catch (IOException e) {
				log.accept("replay: save failed: " + e);
				return null;
			}
		}

		private ReplayMeta meta() {
			ReplayMeta m = new ReplayMeta();
			m.singleplayer = singleplayer;
			m.serverName = server;
			m.duration = lastTime;
			m.date = date;
			m.protocol = protocol;
			m.selfId = selfId;
			m.mcversion = mcVersion;
			return m;
		}

		private ReplayMeta.Extras extras() {
			ReplayMeta.Extras e = new ReplayMeta.Extras();
			e.selfName = selfName;
			e.selfUuid = selfUuid == null ? null : selfUuid.toString();
			e.clipSeconds = clipSeconds.getAsInt();
			synchronized (moments) {
				e.moments = new ArrayList<Integer>(moments);
			}
			return e;
		}

		private String fileName() {
			String when = new SimpleDateFormat("yyyy-MM-dd HH-mm-ss").format(new Date(date));
			String where = server.replaceAll("[^A-Za-z0-9 ._-]", "_").trim();
			if (where.length() > 40) {
				where = where.substring(0, 40).trim();
			}
			return (where.isEmpty() ? "Replay" : where) + " " + when + ".mcpr";
		}
	}

	private static void entry(ZipOutputStream zip, String name, byte[] data) throws IOException {
		zip.putNextEntry(new ZipEntry(name));
		zip.write(data);
		zip.closeEntry();
	}

	/** The packets written to {@code file} so far, uncompressed. */
	static DataInputStream packetReader(File file) throws IOException {
		return new DataInputStream(new InflaterInputStream(new java.io.BufferedInputStream(new FileInputStream(file), BUFFER), new Inflater(true), BUFFER));
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

	/** Each recorded packet's time and category, read front to back (for compaction). */
	static final class Index {
		int count;
		int[] time = new int[1 << 12];
		byte[] category = new byte[1 << 12];
		long[] key = new long[1 << 12];

		static Index of(File file, long length, ReplayBackend.Classifier classifier) throws IOException {
			Index index = new Index();
			long[] out = new long[1];
			long read = 0;
			try (DataInputStream in = packetReader(file)) {
				byte[] buf = new byte[1 << 16];
				while (read < length) {
					int t = in.readInt();
					int len = in.readInt();
					if (len < 0) {
						throw new IOException("damaged recording");
					}
					if (buf.length < len) {
						buf = new byte[len];
					}
					in.readFully(buf, 0, len);
					read += 8 + len;
					out[0] = 0;
					index.add(t, classifier.classify(java.nio.ByteBuffer.wrap(buf, 0, len).slice(), out), out[0]);
				}
			}
			return index;
		}

		private void add(int t, byte c, long k) {
			if (count == time.length) {
				time = java.util.Arrays.copyOf(time, count * 2);
				category = java.util.Arrays.copyOf(category, count * 2);
				key = java.util.Arrays.copyOf(key, count * 2);
			}
			time[count] = t;
			category[count] = c;
			key[count] = k;
			count++;
		}

		/** The first packet at or after {@code t} ({@link #count} if none). */
		int firstAt(int t) {
			int lo = 0;
			int hi = count;
			while (lo < hi) {
				int mid = (lo + hi) >>> 1;
				if (time[mid] < t) {
					lo = mid + 1;
				} else {
					hi = mid;
				}
			}
			return lo;
		}
	}
}
