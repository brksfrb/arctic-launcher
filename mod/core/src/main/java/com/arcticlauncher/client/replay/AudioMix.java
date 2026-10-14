package com.arcticlauncher.client.replay;

import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Small, deterministic stereo blocks on the encoder worker; never hours of PCM on disk. */
final class AudioMix implements Closeable {
	static final int RATE = 48000;
	private final ZipFile zip;
	private final DataInputStream events;
	private final ReplayBackend backend;
	private final Map<String, File> assets = new HashMap<String, File>();
	private final Map<Integer, Voice> voices = new LinkedHashMap<Integer, Voice>();
	private AudioEvent next;
	private long sample;
	private final Map<Integer, AudioEvent> initial = new LinkedHashMap<Integer, AudioEvent>();
	AudioMix(File replay, ReplayBackend backend, int from) throws IOException {
		this.backend = backend;
		zip = new ZipFile(replay);
		ZipEntry entry = zip.getEntry(AudioEvent.ENTRY);
		events = entry == null ? null : new DataInputStream(new BufferedInputStream(zip.getInputStream(entry)));
		sample = (long) from * RATE / 1000;
		advance();
		while (next != null && next.time <= from) {
			if (next.resource.isEmpty()) initial.remove(next.id); else initial.put(next.id, next);
			advance();
		}
	}
	private void advance() throws IOException {
		if (events == null) { next = null; return; }
		try { next = AudioEvent.read(events); } catch (EOFException end) { next = null; }
	}
	/** Always exactly frames of signed 16-bit little-endian stereo, including silence. */
	byte[] block(int frames) throws IOException {
		byte[] out = new byte[frames * 4];
		try {
			for (AudioEvent e : initial.values()) {
				if (voices.size() >= 256) throw new IOException("Too many simultaneous replay sounds");
				voices.put(e.id, new Voice(e.at((int) (sample * 1000 / RATE), 0), asset(e.resource)));
			}
			initial.clear();
			for (int i = 0; i < frames; i++, sample++) {
				double ms = sample * 1000.0 / RATE;
				while (next != null && next.time <= ms) {
					AudioEvent e = next;
					Voice voice = voices.get(e.id);
					if (e.resource.isEmpty()) {
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
		try { if (events != null) events.close(); } finally { zip.close(); }
	}
	private static final class Voice implements Closeable {
		AudioEvent event;
		final AudioDecoder decoder;
		short[] buffer = new short[0];
		double cursor;
		boolean ended;
		float left, right;
		Voice(AudioEvent event, File file) throws Exception {
			this.event = event; decoder = AudioDecoder.open(file);
			double position = event.position;
			if (event.loop && decoder.length > 0) position %= (double) decoder.length / decoder.rate;
			decoder.seek(position);
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
