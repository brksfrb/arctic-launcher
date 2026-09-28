package com.arcticlauncher.client.looks;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * An emote or a cosmetic's idle motion, from a Blockbench "Bedrock
 * animation" file: per bone, rotation and position keyframes that are
 * blended linearly. Expressions (Molang) aren't supported and count as 0.
 */
public final class Animation {
	private static final int MAX_BONES = 64;
	private static final int MAX_KEYFRAMES = 256;
	private static final float MAX_SECONDS = 60f;
	private static final float MAX_ANGLE = 3600f;
	private static final float MAX_OFFSET = 256f;

	/** One channel's keyframes, sorted by time. */
	static final class Track {
		final float[] times;
		final float[][] values;

		Track(float[] times, float[][] values) {
			this.times = times;
			this.values = values;
		}

		/** The value at {@code t} seconds (held before the first and after the last). */
		float[] at(float t) {
			int n = times.length;
			if (n == 1 || t <= times[0]) {
				return values[0];
			}
			if (t >= times[n - 1]) {
				return values[n - 1];
			}
			int i = 1;
			while (times[i] < t) {
				i++;
			}
			float span = times[i] - times[i - 1];
			float k = span <= 0 ? 1f : (t - times[i - 1]) / span;
			float[] a = values[i - 1];
			float[] b = values[i];
			return new float[] {a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k};
		}
	}

	public final float length;
	public final boolean looping;
	private final Map<String, Track> rotations;
	private final Map<String, Track> positions;

	private Animation(float length, boolean looping, Map<String, Track> rotations, Map<String, Track> positions) {
		this.length = length;
		this.looping = looping;
		this.rotations = rotations;
		this.positions = positions;
	}

	/** Parse the first animation in a file; throws with a reason when it can't. */
	public static Animation parse(JsonElement json) {
		JsonObject animations = Geometry.object(Geometry.object(json).get("animations"));
		if (animations.entrySet().isEmpty()) {
			throw new IllegalArgumentException("no animations");
		}
		JsonObject a = Geometry.object(animations.entrySet().iterator().next().getValue());
		float length = Geometry.number(a.get("animation_length"), MAX_SECONDS);
		if (length <= 0) {
			throw new IllegalArgumentException("animation_length must be above 0");
		}
		boolean looping = a.has("loop") && a.get("loop").isJsonPrimitive() && a.get("loop").getAsJsonPrimitive().isBoolean()
				&& a.get("loop").getAsBoolean();
		Map<String, Track> rotations = new HashMap<String, Track>();
		Map<String, Track> positions = new HashMap<String, Track>();
		if (a.has("bones")) {
			JsonObject bones = Geometry.object(a.get("bones"));
			if (bones.entrySet().size() > MAX_BONES) {
				throw new IllegalArgumentException("too many bones");
			}
			for (Map.Entry<String, JsonElement> bone : bones.entrySet()) {
				JsonObject channels = Geometry.object(bone.getValue());
				if (channels.has("rotation")) {
					rotations.put(bone.getKey(), track(channels.get("rotation"), MAX_ANGLE));
				}
				if (channels.has("position")) {
					positions.put(bone.getKey(), track(channels.get("position"), MAX_OFFSET));
				}
			}
		}
		return new Animation(length, looping, Collections.unmodifiableMap(rotations), Collections.unmodifiableMap(positions));
	}

	/** Rotation of {@code bone} in degrees at {@code t} seconds, or null if it isn't animated. */
	public float[] rotation(String bone, float t) {
		Track track = rotations.get(bone);
		return track == null ? null : track.at(time(t));
	}

	/** Position offset of {@code bone} in pixels at {@code t} seconds, or null. */
	public float[] position(String bone, float t) {
		Track track = positions.get(bone);
		return track == null ? null : track.at(time(t));
	}

	/** Whether a one-shot animation is over at {@code t}. */
	public boolean finished(float t) {
		return !looping && t >= length;
	}

	private float time(float t) {
		return looping ? t % length : Math.min(t, length);
	}

	/** A channel: a constant [x, y, z], or keyframes {"0.5": [x, y, z], ...}. */
	private static Track track(JsonElement e, float limit) {
		if (e.isJsonArray()) {
			return new Track(new float[] {0f}, new float[][] {vec(e, limit)});
		}
		JsonObject frames = Geometry.object(e);
		if (frames.entrySet().isEmpty() || frames.entrySet().size() > MAX_KEYFRAMES) {
			throw new IllegalArgumentException("a channel needs 1 to " + MAX_KEYFRAMES + " keyframes");
		}
		List<float[]> rows = new ArrayList<float[]>();
		for (Map.Entry<String, JsonElement> f : frames.entrySet()) {
			float time;
			try {
				time = Float.parseFloat(f.getKey());
			} catch (NumberFormatException ex) {
				throw new IllegalArgumentException("keyframe time " + f.getKey());
			}
			if (!(time >= 0 && time <= MAX_SECONDS)) {
				throw new IllegalArgumentException("keyframe time out of range");
			}
			float[] v = keyframe(f.getValue(), limit);
			rows.add(new float[] {time, v[0], v[1], v[2]});
		}
		Collections.sort(rows, (x, y) -> Float.compare(x[0], y[0]));
		float[] times = new float[rows.size()];
		float[][] values = new float[rows.size()][];
		for (int i = 0; i < rows.size(); i++) {
			times[i] = rows.get(i)[0];
			values[i] = Arrays.copyOfRange(rows.get(i), 1, 4);
		}
		return new Track(times, values);
	}

	/** [x, y, z], or Blockbench's {"pre": ..., "post": ...} (the post value is used). */
	private static float[] keyframe(JsonElement e, float limit) {
		if (e.isJsonObject()) {
			JsonObject o = e.getAsJsonObject();
			JsonElement v = o.has("post") ? o.get("post") : o.get("pre");
			return vec(v, limit);
		}
		return vec(e, limit);
	}

	/** A number written as text ("15"); anything else (Molang) is 0. */
	private static float plainNumber(String s, float limit) {
		try {
			float f = Float.parseFloat(s.trim());
			return Float.isNaN(f) || Math.abs(f) > limit ? 0f : f;
		} catch (NumberFormatException e) {
			return 0f;
		}
	}

	/** [x, y, z] where Molang expressions count as 0. */
	private static float[] vec(JsonElement e, float limit) {
		if (e == null || !e.isJsonArray() || e.getAsJsonArray().size() != 3) {
			throw new IllegalArgumentException("expected [x, y, z]");
		}
		float[] out = new float[3];
		for (int i = 0; i < 3; i++) {
			JsonElement c = e.getAsJsonArray().get(i);
			boolean text = c.isJsonPrimitive() && c.getAsJsonPrimitive().isString();
			out[i] = text ? plainNumber(c.getAsString(), limit) : Geometry.number(c, limit);
		}
		return out;
	}
}
