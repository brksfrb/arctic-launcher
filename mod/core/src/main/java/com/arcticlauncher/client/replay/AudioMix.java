package com.arcticlauncher.client.replay;

import java.io.*;
import java.util.*;
import java.util.function.DoubleUnaryOperator;
import java.util.zip.*;

/** Small, deterministic stereo blocks on the encoder worker; never hours of PCM on disk. */
final class AudioMix implements Closeable {
	static final int RATE = 48000;
	private final ZipFile zip;
	private final DataInputStream events;
	private final ReplayBackend backend;
	private final java.util.function.Supplier<AudioEvent> live;
	private final Map<String, File> assets = new HashMap<String, File>();
	private final Map<Integer, Voice> voices = new LinkedHashMap<Integer, Voice>();
	private AudioEvent next;
	private long sample;
	private final DoubleUnaryOperator timeMap;
	private final List<Change> changes = new ArrayList<Change>();
	private final Map<Integer, AudioEvent> changedVoices = new LinkedHashMap<Integer, AudioEvent>();
	private int change;
	private final Map<Integer, AudioEvent> initial = new LinkedHashMap<Integer, AudioEvent>();
	AudioMix(File replay, ReplayBackend backend, int from, DoubleUnaryOperator timeMap) throws IOException {
		live = null;
		this.timeMap = timeMap;
		this.backend = backend;
		zip = new ZipFile(replay);
		ZipEntry entry = zip.getEntry(AudioEvent.ENTRY);
		events = entry == null ? null : new DataInputStream(new BufferedInputStream(zip.getInputStream(entry)));
		sample = (long) from * RATE / 1000;
		try {
			advance();
			if (timeMap == null) {
				while (next != null && next.time <= from) {
					if (next.resource.isEmpty()) initial.remove(next.id); else initial.put(next.id, next);
					advance();
				}
			} else {
				Map<Integer, AudioEvent> previous = new HashMap<Integer, AudioEvent>();
				while (next != null) {
					AudioEvent before = next.resource.isEmpty() ? previous.remove(next.id) : previous.put(next.id, next);
					changes.add(new Change(before, next));
					advance();
				}
			}
		} catch (IOException | RuntimeException e) {
			close();
			throw e;
		}
	}
	AudioMix(ReplayBackend backend, int from, java.util.function.Supplier<AudioEvent> live) {
		this.backend = backend; this.live = live; zip = null; events = null;
		timeMap = null;
		sample = (long) from * RATE / 1000;
		advanceLive();
	}
	private void advanceLive() { next = live.get(); }
	private void advance() throws IOException {
		if (live != null) { advanceLive(); return; }
		if (events == null) { next = null; return; }
		try { next = AudioEvent.read(events); } catch (EOFException end) { next = null; }
	}
	/** Always exactly frames of signed 16-bit little-endian stereo, including silence. */
	byte[] block(int frames) throws IOException {
		if (timeMap != null) return retimedBlock(frames);
		if (next == null && live != null) advanceLive();
		byte[] out = new byte[frames * 4];
		try {
			for (AudioEvent e : initial.values()) {
				if (e.left < .0001f && e.right < .0001f) continue;
				if (voices.size() >= 256) throw new IOException("Too many simultaneous replay sounds");
				voices.put(e.id, new Voice(e.at((int) (sample * 1000 / RATE), 0), asset(e.resource)));
			}
			initial.clear();
			for (int i = 0; i < frames; i++, sample++) {
				double ms = sample * 1000.0 / RATE;
				while (next != null && next.time <= ms) {
					AudioEvent e = next;
					Voice voice = voices.get(e.id);
					if (e.resource.isEmpty() || (e.left < .0001f && e.right < .0001f)) {
						if (voice != null) { voice.close(); voices.remove(e.id); }
					} else if (voice == null || !voice.event.resource.equals(e.resource)) {
						if (voice != null) voice.close();
						if (voices.size() >= 256) throw new IOException("Too many simultaneous replay sounds");
						voices.put(e.id, new Voice(e.at((int) ms, 0), asset(e.resource)));
					} else voice.event = e;
					advance();
				}
				double left = 0, right = 0;
				for (Voice voice : voices.values()) {
					voice.step();
					left += voice.left * voice.event.left;
					right += voice.right * voice.event.right;
				}
				put(out, i * 4, left); put(out, i * 4 + 2, right);
			}
		} catch (Exception e) { throw e instanceof IOException ? (IOException) e : new IOException("Replay audio: " + e, e); }
		return out;
	}

	private byte[] retimedBlock(int frames) throws IOException {
		byte[] out = new byte[frames * 4];
		try {
			double ms = timeMap.applyAsDouble(sample * 1000.0 / RATE);
			for (int i = 0; i < frames; i++, sample++) {
				double following = timeMap.applyAsDouble((sample + 1) * 1000.0 / RATE);
				// A held frame has no source audio interval.
				if (ms != following) {
					while (change > 0 && changes.get(change - 1).after.time > ms) {
						Change c = changes.get(--change);
						changedVoices.put(c.after.id, c.before);
					}
					while (change < changes.size() && changes.get(change).after.time <= ms) {
						Change c = changes.get(change++);
						changedVoices.put(c.after.id, c.after);
					}
					for (Map.Entry<Integer, AudioEvent> dirty : changedVoices.entrySet()) update(dirty.getKey(), dirty.getValue());
					changedVoices.clear();
					double left = 0, right = 0;
					for (Voice voice : voices.values()) {
						voice.at(ms);
						left += voice.left * voice.event.left;
						right += voice.right * voice.event.right;
					}
					put(out, i * 4, left);
					put(out, i * 4 + 2, right);
				}
				ms = following;
			}
		} catch (Exception e) { throw e instanceof IOException ? (IOException) e : new IOException("Replay audio: " + e, e); }
		return out;
	}

	private void update(int id, AudioEvent event) throws Exception {
		Voice voice = voices.get(id);
		if (event == null || event.resource.isEmpty() || (event.left < .0001f && event.right < .0001f)) {
			if (voice != null) { voices.remove(id); voice.close(); }
		} else if (voice != null && voice.event.resource.equals(event.resource)) {
			voice.event = event;
		} else {
			if (voice != null) { voices.remove(id); voice.close(); }
			if (voices.size() >= 256) throw new IOException("Too many simultaneous replay sounds");
			voices.put(id, new Voice(event, asset(event.resource), false));
		}
	}

	private static final class Change {
		final AudioEvent before, after;
		Change(AudioEvent before, AudioEvent after) { this.before = before; this.after = after; }
	}
	private static void put(byte[] out, int at, double value) {
		int n = (int) Math.max(-32768, Math.min(32767, value)); out[at] = (byte) n; out[at + 1] = (byte) (n >> 8);
	}
	private File asset(String name) throws IOException {
		File found = assets.get(name); if (found != null) return found;
		// Bound temporary compressed assets even for very long sessions with many custom sounds.
		long cached = 0;
		for (File f : assets.values()) cached += f.length();
		if (cached > (128L << 20)) {
			Set<String> used = new HashSet<String>();
			for (Voice v : voices.values()) used.add(v.event.resource);
			Iterator<Map.Entry<String, File>> it = assets.entrySet().iterator();
			while (it.hasNext()) {
				Map.Entry<String, File> e = it.next();
				if (!used.contains(e.getKey())) { e.getValue().delete(); it.remove(); }
			}
		}
		File file = File.createTempFile("arctic-replay-sound-", ".ogg");
		try (InputStream in = backend.audioResource(name); OutputStream out = new BufferedOutputStream(new FileOutputStream(file))) {
			byte[] buffer = new byte[16384]; long size = 0; int n;
			while ((n = in.read(buffer)) >= 0) {
				size += n; if (size > (64L << 20)) throw new IOException("Replay sound is too large: " + name);
				out.write(buffer, 0, n);
			}
		} catch (IOException e) { file.delete(); throw e; }
		assets.put(name, file); return file;
	}
	@Override public void close() throws IOException {
		for (Voice voice : voices.values()) try { voice.close(); } catch (IOException ignored) {}
		voices.clear();
		for (File asset : assets.values()) asset.delete();
		assets.clear();
		try { if (events != null) events.close(); } finally { if (zip != null) zip.close(); }
	}
	private static final class Voice implements Closeable {
		private static final int TILE = 4096;
		AudioEvent event;
		final AudioDecoder decoder;
		short[] buffer = new short[0];
		double cursor;
		boolean ended;
		float left, right;
		long bufferStart = -1;
		long decoderAt;
		Voice(AudioEvent event, File file) throws Exception {
			this(event, file, true);
		}
		Voice(AudioEvent event, File file, boolean sequential) throws Exception {
			this.event = event; decoder = AudioDecoder.open(file);
			if (sequential) {
				double position = event.position;
				if (event.loop && decoder.length > 0) position %= (double) decoder.length / decoder.rate;
				decoder.seek(position);
			}
		}
		void at(double ms) throws Exception {
			long frame = (long) ((event.position + (ms - event.time) * event.pitch / 1000) * decoder.rate);
			if (event.loop && decoder.length > 0) frame %= decoder.length;
			left = right = 0;
			if (frame < 0 || (decoder.length > 0 && frame >= decoder.length)) return;
			if (frame < bufferStart || frame >= bufferStart + buffer.length / decoder.channels) {
				long begin = frame / TILE * TILE;
				if (begin != decoderAt) decoder.seekFrame(begin);
				bufferStart = begin;
				buffer = new short[TILE * decoder.channels];
				int written = 0;
				while (written < buffer.length) {
					short[] decoded = decoder.read();
					if (decoded.length == 0) break;
					int count = Math.min(decoded.length, buffer.length - written);
					System.arraycopy(decoded, 0, buffer, written, count);
					written += count;
					begin += decoded.length / decoder.channels;
				}
				decoderAt = begin;
			}
			int at = (int) (frame - bufferStart) * decoder.channels;
			left = buffer[at];
			right = buffer[at + (decoder.channels > 1 ? 1 : 0)];
		}
		void step() throws Exception {
			if (ended) { left = right = 0; return; }
			while ((int) cursor >= buffer.length / decoder.channels) {
				cursor -= buffer.length / decoder.channels;
				buffer = decoder.read();
				if (buffer.length == 0) {
					if (event.loop) { decoder.seek(0); buffer = decoder.read(); cursor = 0; }
					if (buffer.length == 0) { ended = true; left = right = 0; return; }
				}
			}
			int at = (int) cursor * decoder.channels;
			left = buffer[at]; right = buffer[at + (decoder.channels > 1 ? 1 : 0)];
			cursor += (double) decoder.rate / RATE * event.pitch;
		}
		@Override public void close() throws IOException { decoder.close(); }
	}
}
