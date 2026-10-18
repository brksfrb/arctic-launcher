package com.arcticlauncher.client;

/**
 * How much of the last frame motion blur keeps. A fixed share per frame makes the trail as long as the
 * frame rate is low (a smear at 60 FPS, nothing at 300), so the share follows from the frame time:
 * each strength has a trail half-life in milliseconds, and the share is 0.5 ^ (frame ms / half-life).
 * The post-effect paths (26.1+) can't change a uniform per frame, so they pick one of effects baked
 * for {@link #BUCKET_MS} frame times (mod/tools/motion_blur.py writes them; keep the numbers in step).
 */
public final class MotionBlurTiming {
	/** Trail half-life in ms for strength 1 (Low) to 4 (Max). */
	public static final float[] HALF_LIFE_MS = {12f, 20f, 30f, 45f};
	/** Frame times the baked effects stand for, ascending. */
	public static final float[] BUCKET_MS = {3f, 5f, 8f, 11f, 14f, 17f, 22f, 30f};

	private static final float MIN_MS = 1f;
	private static final float MAX_MS = 50f;
	/** How fast the measured frame time follows new frames (a steady value, not a jittery one). */
	private static final float SMOOTHING = 0.1f;

	private static long lastNanos;
	private static float frameMs = 16f;

	private MotionBlurTiming() {}

	/** Call once per frame; returns the smoothed frame time in ms. */
	public static synchronized float tick() {
		long now = System.nanoTime();
		if (lastNanos != 0) {
			float ms = Math.max(MIN_MS, Math.min(MAX_MS, (now - lastNanos) / 1_000_000f));
			frameMs += (ms - frameMs) * SMOOTHING;
		}
		lastNanos = now;
		return frameMs;
	}

	/** Forget the last frame (blur was off for a while): the next frame time isn't a long gap. */
	public static synchronized void reset() {
		lastNanos = 0;
	}

	/** The share of the last frame to keep, for a strength of 1 to 4 and a frame time in ms. */
	public static float weight(int strength, float frameMs) {
		int level = Math.max(1, Math.min(HALF_LIFE_MS.length, strength));
		return (float) Math.pow(0.5, frameMs / HALF_LIFE_MS[level - 1]);
	}

	/** The baked effect closest to this frame time. */
	public static int bucket(float frameMs) {
		int best = 0;
		for (int i = 1; i < BUCKET_MS.length; i++) {
			if (Math.abs(BUCKET_MS[i] - frameMs) < Math.abs(BUCKET_MS[best] - frameMs)) {
				best = i;
			}
		}
		return best;
	}

	/** Name suffix of the baked effect: strength 1-4, bucket index. */
	public static String effectSuffix(int strength, float frameMs) {
		int level = Math.max(1, Math.min(HALF_LIFE_MS.length, strength));
		return level + "_" + bucket(frameMs);
	}
}
