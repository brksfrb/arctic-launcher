package com.arcticlauncher.client.replay;

import java.io.*;
import java.lang.reflect.*;
import java.nio.*;

/** Streaming Vorbis decoding through libraries Minecraft already ships; no extra native download. */
abstract class AudioDecoder implements Closeable {
	int channels, rate;
	long length;
	abstract short[] read() throws Exception;
	abstract void seek(double seconds) throws Exception;
	static AudioDecoder open(File file) throws Exception {
		try { Class.forName("org.lwjgl.stb.STBVorbis"); return new Stb(file); }
		catch (ClassNotFoundException oldMinecraft) { return new Legacy(file); }
	}
	private static final class Stb extends AudioDecoder {
		private final Class<?> api = Class.forName("org.lwjgl.stb.STBVorbis");
		private final long handle;
		private final Method read, seek, close;
		private final ShortBuffer buffer;
		Stb(File file) throws Exception {
			IntBuffer error = ByteBuffer.allocateDirect(4).order(ByteOrder.nativeOrder()).asIntBuffer();
			handle = (Long) api.getMethod("stb_vorbis_open_filename", CharSequence.class, IntBuffer.class,
					Class.forName("org.lwjgl.stb.STBVorbisAlloc")).invoke(null, file.getAbsolutePath(), error, null);
			if (handle == 0) throw new IOException("Cannot decode game sound (Vorbis " + error.get(0) + ")");
			Class<?> infoClass = Class.forName("org.lwjgl.stb.STBVorbisInfo");
			Object info = infoClass.getMethod("malloc").invoke(null);
			try {
				api.getMethod("stb_vorbis_get_info", long.class, infoClass).invoke(null, handle, info);
				channels = (Integer) infoClass.getMethod("channels").invoke(info);
				rate = (Integer) infoClass.getMethod("sample_rate").invoke(info);
			} finally { infoClass.getMethod("free").invoke(info); }
			length = (Integer) api.getMethod("stb_vorbis_stream_length_in_samples", long.class).invoke(null, handle);
			read = api.getMethod("stb_vorbis_get_samples_short_interleaved", long.class, int.class, ShortBuffer.class);
			seek = api.getMethod("stb_vorbis_seek", long.class, int.class);
			close = api.getMethod("stb_vorbis_close", long.class);
			buffer = ByteBuffer.allocateDirect(4096 * channels * 2).order(ByteOrder.nativeOrder()).asShortBuffer();
		}
		@Override short[] read() throws Exception {
			buffer.clear();
			int n = (Integer) read.invoke(null, handle, channels, buffer);
			short[] data = new short[n * channels]; buffer.get(data); return data;
		}
		@Override void seek(double seconds) throws Exception {
			long sample = Math.max(0, (long) (seconds * rate));
			seek.invoke(null, handle, (int) Math.min(Math.max(0, length), sample));
		}
		@Override public void close() throws IOException {
			try { close.invoke(null, handle); } catch (Exception e) { throw new IOException(e); }
		}
	}
	private static final class Legacy extends AudioDecoder {
		private final Object codec;
		private final Class<?> api;
		private final File file;
		private short[] pending;
		Legacy(File file) throws Exception {
			this.file = file;
			api = Class.forName("paulscode.sound.codecs.CodecJOrbis"); codec = api.newInstance();
			Class<?> vorbis = Class.forName("com.jcraft.jorbis.VorbisFile");
			Object info = vorbis.getConstructor(String.class).newInstance(file.getAbsolutePath());
			try { length = (Long) vorbis.getMethod("pcm_total", int.class).invoke(info, -1); }
			finally { vorbis.getMethod("close").invoke(info); }
			init();
		}
		private void init() throws Exception {
			if (!(Boolean) api.getMethod("initialize", java.net.URL.class).invoke(codec, file.toURI().toURL())) throw new IOException("Cannot decode game sound");
			javax.sound.sampled.AudioFormat format = (javax.sound.sampled.AudioFormat) api.getMethod("getAudioFormat").invoke(codec);
			channels = format.getChannels(); rate = (int) format.getSampleRate();
		}
		@Override short[] read() throws Exception {
			if (pending != null) { short[] p = pending; pending = null; return p; }
			Object result = api.getMethod("read").invoke(codec);
			if (result == null) return new short[0];
			byte[] raw = (byte[]) result.getClass().getField("audioData").get(result);
			javax.sound.sampled.AudioFormat f = (javax.sound.sampled.AudioFormat) result.getClass().getField("audioFormat").get(result);
			short[] data = new short[raw.length / 2];
			ByteBuffer.wrap(raw).order(f.isBigEndian() ? ByteOrder.BIG_ENDIAN : ByteOrder.LITTLE_ENDIAN).asShortBuffer().get(data);
			return data;
		}
		@Override void seek(double seconds) throws Exception {
			api.getMethod("cleanup").invoke(codec); pending = null; init();
			long skip = Math.max(0, (long) (seconds * rate)) * channels;
			while (skip > 0) {
				short[] data = read(); if (data.length == 0) return;
				if (data.length > skip) { pending = java.util.Arrays.copyOfRange(data, (int) skip, data.length); return; }
				skip -= data.length;
			}
		}
		@Override public void close() throws IOException {
			try { api.getMethod("cleanup").invoke(codec); } catch (Exception e) { throw new IOException(e); }
		}
	}
}
