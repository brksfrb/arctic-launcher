package com.arcticlauncher.client.replay;

import com.google.gson.Gson;
import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.Closeable;
import java.io.DataInputStream;
import java.io.EOFException;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * A replay opened for watching: its packets unpacked once into a cache
 * file (so jumping anywhere reads straight from disk), with an index of
 * where each packet starts and when it arrived.
 */
public final class ReplayData implements Closeable {
	private static final Gson GSON = new Gson();
	private static final int HEADER = 8;

	public final File file;
	public final ReplayMeta meta;
	public final ReplayMeta.Extras extras;
	private final RandomAccessFile raf;
	private final FileChannel channel;
	private final File cacheFile;
	/** Packet count, and per packet: arrival (ms), where its bytes start, their length. */
	final int count;
	final int[] time;
	final int[] offset;
	final int[] length;
	/** The recording player, once a tick. */
	final SelfSample[] self;
	/** Filled in by {@link #classify} (null until then). */
	volatile byte[] category;
	volatile long[] key;

	private ReplayData(File file, ReplayMeta meta, ReplayMeta.Extras extras, RandomAccessFile raf, File cacheFile,
			int count, int[] time, int[] offset, int[] length, SelfSample[] self) {
		this.file = file;
		this.meta = meta;
		this.extras = extras;
		this.raf = raf;
		this.channel = raf.getChannel();
		this.cacheFile = cacheFile;
		this.count = count;
		this.time = time;
		this.offset = offset;
		this.length = length;
		this.self = self;
	}

	/** Open {@code mcpr}, unpacking its packets into {@code cacheDir} (reused while unchanged). */
	public static ReplayData open(File mcpr, File cacheDir) throws IOException {
		ReplayMeta meta;
		ReplayMeta.Extras extras;
		SelfSample[] self;
		File packets = new File(cacheDir, cacheName(mcpr));
		try (ZipFile zip = new ZipFile(mcpr)) {
			meta = GSON.fromJson(text(zip, Recorder.META_ENTRY), ReplayMeta.class);
			if (meta == null) {
				throw new IOException("not a replay (no metaData.json)");
			}
			String extrasJson = text(zip, Recorder.EXTRAS_ENTRY);
			extras = extrasJson == null ? new ReplayMeta.Extras() : GSON.fromJson(extrasJson, ReplayMeta.Extras.class);
			extras.fillDefaults();
			self = readSelf(zip);
			if (!packets.isFile()) {
				unpack(zip, packets, cacheDir);
			}
		}
		RandomAccessFile raf = new RandomAccessFile(packets, "r");
		try {
			long size = raf.length();
			if (size > Integer.MAX_VALUE) {
				throw new IOException("replay too large (" + (size >> 20) + " MB)");
			}
			return index(mcpr, meta, extras, raf, packets, self);
		} catch (IOException | RuntimeException e) {
			raf.close();
			throw e;
		}
	}

	/** Cache file name: changes when the replay file does (it's rewritten on each save). */
	private static String cacheName(File mcpr) {
		String key = mcpr.getAbsolutePath() + "|" + mcpr.length() + "|" + mcpr.lastModified();
		return Integer.toHexString(key.hashCode()) + ".tmcpr";
	}

	/** Unpack the packets, dropping older cache files (one replay's worth of disk at a time). */
	private static void unpack(ZipFile zip, File to, File cacheDir) throws IOException {
		cacheDir.mkdirs();
		File[] old = cacheDir.listFiles();
		if (old != null) {
			for (File f : old) {
				f.delete();
			}
		}
		ZipEntry entry = zip.getEntry(Recorder.PACKETS_ENTRY);
		if (entry == null) {
			throw new IOException("replay has no recording");
		}
		File part = new File(to.getPath() + ".part");
		try (InputStream in = zip.getInputStream(entry); OutputStream out = new BufferedOutputStream(new FileOutputStream(part), 1 << 18)) {
			Recorder.copyAll(in, out);
		}
		if (!part.renameTo(to)) {
			throw new IOException("could not write " + to);
		}
	}

	/** Read every packet's header, front to back (the bodies are skipped over). */
	private static ReplayData index(File mcpr, ReplayMeta meta, ReplayMeta.Extras extras, RandomAccessFile raf, File cacheFile, SelfSample[] self)
			throws IOException {
		int cap = 1 << 16;
		int[] time = new int[cap];
		int[] offset = new int[cap];
		int[] length = new int[cap];
		int n = 0;
		long limit = raf.length();
		int at = 0;
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(new java.io.FileInputStream(cacheFile), 1 << 16))) {
			while (at + HEADER <= limit) {
				int t = in.readInt();
				int len = in.readInt();
				if (len < 0 || at + HEADER + (long) len > limit) {
					// A cut-off last packet (the game closed mid-write).
					break;
				}
				if (n == cap) {
					cap *= 2;
					time = Arrays.copyOf(time, cap);
					offset = Arrays.copyOf(offset, cap);
					length = Arrays.copyOf(length, cap);
				}
				time[n] = t;
				offset[n] = at + HEADER;
				length[n] = len;
				n++;
				at += HEADER + len;
				int skipped = 0;
				while (skipped < len) {
					int k = in.skipBytes(len - skipped);
					if (k <= 0) {
						break;
					}
					skipped += k;
				}
			}
		}
		if (n > 0 && meta.duration < time[n - 1]) {
			meta.duration = time[n - 1];
		}
		return new ReplayData(mcpr, meta, extras, raf, cacheFile, n, time, offset, length, self);
	}

	private static SelfSample[] readSelf(ZipFile zip) throws IOException {
		ZipEntry entry = zip.getEntry(Recorder.SELF_ENTRY);
		if (entry == null) {
			return new SelfSample[0];
		}
		List<SelfSample> out = new ArrayList<SelfSample>();
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(zip.getInputStream(entry)))) {
			while (true) {
				out.add(SelfSample.read(in));
			}
		} catch (EOFException end) {
			// Done (a cut-off last sample is dropped).
		}
		return out.toArray(new SelfSample[0]);
	}

	private static String text(ZipFile zip, String name) throws IOException {
		ZipEntry entry = zip.getEntry(name);
		if (entry == null) {
			return null;
		}
		try (InputStream in = zip.getInputStream(entry)) {
			ByteArrayOutputStream out = new ByteArrayOutputStream();
			Recorder.copyAll(in, out);
			return new String(out.toByteArray(), StandardCharsets.UTF_8);
		}
	}

	public int count() {
		return count;
	}

	public int duration() {
		return meta.duration;
	}

	/** When packet {@code i} arrived (ms). */
	public int time(int i) {
		return time[i];
	}

	/** Packet {@code i}'s bytes (its id, then its body), freshly read (the game may keep them). */
	public ByteBuffer packet(int i) {
		ByteBuffer b = ByteBuffer.allocate(length[i]);
		try {
			while (b.hasRemaining()) {
				if (channel.read(b, offset[i] + b.position()) < 0) {
					break;
				}
			}
		} catch (IOException e) {
			throw new java.io.UncheckedIOException(e);
		}
		b.flip();
		return b;
	}

	/** The first packet that arrives after {@code ms}. */
	public int indexAfter(double ms) {
		int lo = 0;
		int hi = count;
		while (lo < hi) {
			int mid = (lo + hi) >>> 1;
			if (time[mid] <= ms) {
				lo = mid + 1;
			} else {
				hi = mid;
			}
		}
		return lo;
	}

	/** Sort every packet (on a background thread, once), reading the file front to back. */
	public void classify(ReplayBackend.Classifier classifier) {
		byte[] cat = new byte[count];
		long[] keys = new long[count];
		long[] out = new long[1];
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(new java.io.FileInputStream(cacheFile), 1 << 18))) {
			byte[] buf = new byte[1 << 16];
			for (int i = 0; i < count; i++) {
				in.readInt();
				int len = in.readInt();
				if (buf.length < len) {
					buf = new byte[len];
				}
				in.readFully(buf, 0, len);
				out[0] = 0;
				cat[i] = classifier.classify(ByteBuffer.wrap(buf, 0, len).slice(), out);
				keys[i] = out[0];
			}
		} catch (IOException | RuntimeException e) {
			// Jumps stay on the slow path.
			return;
		}
		key = keys;
		category = cat;
	}

	public boolean classified() {
		return category != null;
	}

	/** The recording player at {@code ms}, between the two nearest ticks; null if none recorded. */
	public SelfSample selfAt(double ms) {
		if (self.length == 0) {
			return null;
		}
		int lo = 0;
		int hi = self.length;
		while (lo < hi) {
			int mid = (lo + hi) >>> 1;
			if (self[mid].time <= ms) {
				lo = mid + 1;
			} else {
				hi = mid;
			}
		}
		if (lo == 0) {
			return self[0];
		}
		if (lo == self.length) {
			return self[self.length - 1];
		}
		SelfSample a = self[lo - 1];
		SelfSample b = self[lo];
		float t = (float) ((ms - a.time) / Math.max(1, b.time - a.time));
		return lerp(a, b, t);
	}

	/** The sample at or before {@code ms} (for the discrete bits: swings, slot). */
	public int selfIndexAt(double ms) {
		int lo = 0;
		int hi = self.length;
		while (lo < hi) {
			int mid = (lo + hi) >>> 1;
			if (self[mid].time <= ms) {
				lo = mid + 1;
			} else {
				hi = mid;
			}
		}
		return lo - 1;
	}

	public SelfSample selfSample(int i) {
		return i >= 0 && i < self.length ? self[i] : null;
	}

	private static SelfSample lerp(SelfSample a, SelfSample b, float t) {
		SelfSample s = new SelfSample();
		s.time = (int) (a.time + (b.time - a.time) * t);
		s.x = a.x + (b.x - a.x) * t;
		s.y = a.y + (b.y - a.y) * t;
		s.z = a.z + (b.z - a.z) * t;
		s.yaw = angle(a.yaw, b.yaw, t);
		s.pitch = a.pitch + (b.pitch - a.pitch) * t;
		s.headYaw = angle(a.headYaw, b.headYaw, t);
		s.bodyYaw = angle(a.bodyYaw, b.bodyYaw, t);
		s.flags = a.flags;
		s.slot = a.slot;
		s.health = a.health;
		s.food = a.food;
		s.fov = a.fov;
		return s;
	}

	private static float angle(float a, float b, float t) {
		float d = b - a;
		d -= 360f * Math.round(d / 360f);
		return a + d * t;
	}

	@Override
	public void close() throws IOException {
		raf.close();
	}
}
