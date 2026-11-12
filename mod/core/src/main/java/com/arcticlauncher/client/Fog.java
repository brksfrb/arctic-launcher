package com.arcticlauncher.client;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * The Fog feature's arithmetic, shared by every version module. Hooks must only scale the distances the game is
 * about to set (never read the GL fog state back and multiply it again: that compounds every frame), and
 * the result must stay a finite number, or the whole world is fogged.
 */
public final class Fog {
	/** "No fog at all": the fog distances pushed this far (finite, so the GL fog equation stays defined). */
	public static final float NO_FOG_FACTOR = 1000f;
	/** The largest distance handed to the game; no view is anywhere near this long. */
	public static final float MAX_DISTANCE = 1.0e7f;

	private Fog() {}

	/** The factor for the current settings; 1 (vanilla) when Fog is off or there is no config yet. */
	public static float factor() {
		return factor(ArcticClient.config());
	}

	public static float factor(ClientConfig c) {
		if (c == null || !c.fog) {
			return 1f;
		}
		if (c.fogOff) {
			return NO_FOG_FACTOR;
		}
		double scale = c.fogScale;
		return Double.isNaN(scale) ? 1f : (float) Math.max(1.0, Math.min(10.0, scale));
	}

	/**
	 * A fog distance stretched by {@code factor}. Unchanged when the factor is 1, when the distance is not a plain
	 * finite one (vanilla's "no fog" Float.MAX_VALUE, NaN) or negative; capped at {@link #MAX_DISTANCE}.
	 */
	public static float stretch(float distance, float factor) {
		if (factor == 1f || !(distance > 0f) || distance >= MAX_DISTANCE || !Float.isFinite(distance)) {
			return distance;
		}
		return Math.min(distance * factor, MAX_DISTANCE);
	}

	public static float stretch(float distance) {
		return stretch(distance, factor());
	}
}
