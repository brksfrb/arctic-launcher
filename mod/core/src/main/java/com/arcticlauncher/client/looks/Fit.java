package com.arcticlauncher.client.looks;

/**
 * How a worn cosmetic sits, as its wearer set it: moved in half pixels (a face item onto their
 * skin's eyes), or worn on the other side, mirrored (a shoulder pet).
 */
public final class Fit {
	public static final Fit NONE = new Fit(0, 0, false);
	/** The server's limits (half pixels). */
	static final int MAX_UP = 6;
	static final int MAX_FORWARD = 4;

	/** Half pixels up. */
	public final int up;
	/** Half pixels toward the face. */
	public final int forward;
	public final boolean mirror;

	/** A fit by hand (self-tests). */
	public static Fit of(int up, int forward, boolean mirror) {
		return new Fit(up, forward, mirror);
	}

	Fit(int up, int forward, boolean mirror) {
		this.up = Math.max(-MAX_UP, Math.min(MAX_UP, up));
		this.forward = Math.max(-MAX_FORWARD, Math.min(MAX_FORWARD, forward));
		this.mirror = mirror;
	}

	public boolean moves() {
		return up != 0 || forward != 0 || mirror;
	}

	/** The move in the game's model space (1/16 block units; y down, the face toward -z). */
	public float dy() {
		return -up * 0.5f;
	}

	public float dz() {
		return -forward * 0.5f;
	}

	/** {@code {"id": {"up": 1, "forward": -2, "mirror": true}}}: unknown or broken entries are left out. */
	static java.util.Map<String, Fit> parse(com.google.gson.JsonElement e) {
		java.util.Map<String, Fit> out = new java.util.HashMap<String, Fit>();
		if (e == null || !e.isJsonObject()) {
			return java.util.Collections.emptyMap();
		}
		for (java.util.Map.Entry<String, com.google.gson.JsonElement> en : e.getAsJsonObject().entrySet()) {
			if (!en.getValue().isJsonObject()) {
				continue;
			}
			com.google.gson.JsonObject o = en.getValue().getAsJsonObject();
			Fit f = new Fit(number(o, "up"), number(o, "forward"), flag(o, "mirror"));
			if (f.moves()) {
				out.put(en.getKey(), f);
			}
		}
		return java.util.Collections.unmodifiableMap(out);
	}

	private static int number(com.google.gson.JsonObject o, String key) {
		com.google.gson.JsonElement v = o.get(key);
		try {
			return v != null && v.isJsonPrimitive() ? v.getAsInt() : 0;
		} catch (RuntimeException e) {
			return 0;
		}
	}

	private static boolean flag(com.google.gson.JsonObject o, String key) {
		com.google.gson.JsonElement v = o.get(key);
		return v != null && v.isJsonPrimitive() && v.getAsJsonPrimitive().isBoolean() && v.getAsBoolean();
	}
}
