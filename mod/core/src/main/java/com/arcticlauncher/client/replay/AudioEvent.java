package com.arcticlauncher.client.replay;

import java.io.*;
import java.util.LinkedHashMap;
import java.util.Map;

/** A resolved game sound and its playback position. Empty resource means stop. */
public final class AudioEvent {
	public static final String ENTRY = "arctic/audio.bin";
	public int time, id;
	public String resource = "";
	public double position;
	public float left, right, pitch = 1;
	public boolean loop;
	public AudioEvent at(int time, int base) {
		AudioEvent e = new AudioEvent();
		e.time = time - base; e.id = id; e.resource = resource;
		e.position = position + Math.max(0, time - this.time) * pitch / 1000.0;
		e.left = left; e.right = right; e.pitch = pitch; e.loop = loop;
		return e;
	}
	public void write(DataOutput out) throws IOException {
		out.writeInt(time); out.writeInt(id); out.writeUTF(resource); out.writeDouble(position);
		out.writeFloat(left); out.writeFloat(right); out.writeFloat(pitch); out.writeBoolean(loop);
	}
	public static AudioEvent read(DataInput in) throws IOException {
		AudioEvent e = new AudioEvent();
		e.time = in.readInt(); e.id = in.readInt(); e.resource = in.readUTF(); e.position = in.readDouble();
		e.left = in.readFloat(); e.right = in.readFloat(); e.pitch = in.readFloat(); e.loop = in.readBoolean();
		if (!Double.isFinite(e.position) || e.position < 0 || !Float.isFinite(e.pitch) || e.pitch <= 0 || e.pitch > 4
				|| !Float.isFinite(e.left) || !Float.isFinite(e.right) || e.left < 0 || e.right < 0 || e.left > 4 || e.right > 4)
			throw new IOException("Invalid replay audio event");
		return e;
	}
	/** Clip with a checkpoint: already-playing music/loops continue at their original position. */
	static void copy(DataInputStream in, long count, DataOutputStream out, int from, int to, int base,
			java.util.function.LongConsumer copied) throws IOException {
		Map<Integer, AudioEvent> active = new LinkedHashMap<Integer, AudioEvent>();
		boolean seeded = false;
		long written = 0;
		for (long i = 0; i < count; i++) {
			AudioEvent e = read(in);
			if (e.time < from) {
				if (e.resource.isEmpty()) active.remove(e.id); else active.put(e.id, e);
				continue;
			}
			if (!seeded) {
				for (AudioEvent a : active.values()) { a.at(from, base).write(out); written++; }
				seeded = true;
			}
			if (e.time > to) break;
			e.at(e.time, base).write(out); written++;
		}
		if (!seeded) for (AudioEvent a : active.values()) { a.at(from, base).write(out); written++; }
		if (copied != null) copied.accept(written);
	}
}
