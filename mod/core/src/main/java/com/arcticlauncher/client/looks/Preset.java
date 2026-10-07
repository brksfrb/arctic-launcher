package com.arcticlauncher.client.looks;

/** A preset cape everyone can wear. */
public final class Preset {
	public String id;
	public String name;
	/** Texture hash. */
	public String texture;
	/** Frames stacked in the image (1 or missing: a still cape). */
	public int frames;
	/** Frames per second when animated (missing: 8). */
	public int fps;
	/** The still image's hash (an animated cape's "Animate off" look), or null. */
	public String still;

	/** Whether {@code hash} is this cape, animated or its still. */
	public boolean has(String hash) {
		return hash != null && (hash.equals(texture) || hash.equals(still));
	}

	/** Animated, with a still to switch to. */
	public boolean hasStill() {
		return frames > 1 && still != null;
	}
}
