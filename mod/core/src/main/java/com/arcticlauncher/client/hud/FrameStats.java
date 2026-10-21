package com.arcticlauncher.client.hud;

import java.util.Arrays;

/**
 * How long the last frames took: the average frame time and the "1% low" (the frame rate of the
 * slowest hundredth of frames), which is where stutter shows when the average FPS looks fine.
 * Fed once per HUD draw; the numbers are worked out twice a second, not every frame.
 */
public final class FrameStats {
	private static final int KEEP = 600;
	/** A longer gap is a pause (a menu, a loading screen), not a slow frame. */
	private static final long MAX_FRAME_NS = 1_000_000_000L;
	private static final long REFRESH_NS = 500_000_000L;

	private static final long[] FRAMES = new long[KEEP];
	private static int count;
	private static int next;
	private static long last;
	private static long computedAt;
	private static double averageMs = -1;
	private static double lowFps = -1;

	private FrameStats() {}

	/** A frame is being drawn (game thread). */
	public static void tick() {
		tick(System.nanoTime());
	}

	static synchronized void tick(long now) {
		if (last != 0) {
			long d = now - last;
			if (d > 0 && d <= MAX_FRAME_NS) {
				FRAMES[next] = d;
				next = (next + 1) % KEEP;
				count = Math.min(count + 1, KEEP);
			}
		}
		last = now;
	}

	/** Average frame time in ms over the last frames, or -1 with none yet. */
	public static synchronized double averageMs() {
		refresh(System.nanoTime());
		return averageMs;
	}

	/** The 1% low in frames per second, or -1 with too few frames. */
	public static synchronized double lowFps() {
		refresh(System.nanoTime());
		return lowFps;
	}

	static synchronized void reset() {
		count = 0;
		next = 0;
		last = 0;
		computedAt = 0;
		averageMs = -1;
		lowFps = -1;
	}

	private static void refresh(long now) {
		if (now - computedAt < REFRESH_NS && computedAt != 0) {
			return;
		}
		computedAt = now;
		if (count < 20) {
			averageMs = -1;
			lowFps = -1;
			return;
		}
		long[] sorted = Arrays.copyOf(FRAMES, count);
		Arrays.sort(sorted);
		long sum = 0;
		for (long d : sorted) {
			sum += d;
		}
		averageMs = sum / (double) count / 1e6;
		int worst = Math.max(1, count / 100);
		long slow = 0;
		for (int i = count - worst; i < count; i++) {
			slow += sorted[i];
		}
		lowFps = 1e9 / (slow / (double) worst);
	}
}
