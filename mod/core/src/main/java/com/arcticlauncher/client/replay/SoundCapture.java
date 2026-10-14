package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.ArcticClient;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.Map;

/** Samples the game's actual active sounds, including client-only effects and music. */
public final class SoundCapture {
	private final Map<Object, AudioEvent> active = new IdentityHashMap<Object, AudioEvent>();
	private final Map<Object, Boolean> seen = new IdentityHashMap<Object, Boolean>();
	private Recorder.Take take;
	private int nextId;
	public boolean begin() {
		Recorder r = Replays.recorder();
		Recorder.Take current = r == null ? null : r.current();
		if (current != take) { active.clear(); take = current; }
		seen.clear();
		return take != null && !Replays.watching();
	}
	public void sound(Object key, String resource, float volume, float pitch, boolean loop,
			double x, double y, double z, boolean relative, float range) {
		if (take == null || resource == null) return;
		seen.put(key, Boolean.TRUE);
		float left = Math.max(0, Math.min(1, volume)), right = left;
		double[] camera = ArcticClient.platform().camera();
		if (!relative && camera != null && range > 0) {
			double dx = x - camera[0], dy = y - camera[1], dz = z - camera[2];
			double distance = Math.sqrt(dx * dx + dy * dy + dz * dz);
			double yaw = Math.toRadians(camera[3]);
			double pan = distance < 0.01 ? 0 : Math.max(-1, Math.min(1, (-Math.cos(yaw) * dx - Math.sin(yaw) * dz) / distance));
			double gain = Math.max(0, 1 - distance / range);
			left *= gain * Math.sqrt((1 - pan) / 2); right *= gain * Math.sqrt((1 + pan) / 2);
		}
		int now = take.time();
		AudioEvent old = active.get(key);
		pitch = Math.max(0.5f, Math.min(2, pitch));
		if (old != null && old.resource.equals(resource) && old.loop == loop
				&& Math.abs(old.left - left) < 0.01 && Math.abs(old.right - right) < 0.01 && Math.abs(old.pitch - pitch) < 0.001) return;
		AudioEvent e = old == null ? new AudioEvent() : old.at(now, 0);
		e.time = now; e.id = old == null ? ++nextId : old.id; e.resource = resource;
		e.left = left; e.right = right; e.pitch = pitch; e.loop = loop;
		active.put(key, e); take.audio(e);
	}
	public void end() {
		if (take == null) return;
		Iterator<Map.Entry<Object, AudioEvent>> it = active.entrySet().iterator();
		while (it.hasNext()) {
			Map.Entry<Object, AudioEvent> entry = it.next();
			if (seen.containsKey(entry.getKey())) continue;
			AudioEvent e = new AudioEvent(); e.id = entry.getValue().id; e.time = take.time();
			take.audio(e); it.remove();
		}
	}
}
