package com.arcticlauncher.client.replay;

/**
 * The clock the game ticks by while a replay is open: real time scaled by
 * the replay's speed (stopped when paused), or exact steps while exporting
 * video. Version adapters pass the game timer's millisecond reading through
 * {@link #gameMillis}; outside replays it comes back unchanged.
 */
public final class ReplayClock {
	private static volatile boolean active;
	private static volatile double scale = 1;
	/** Exporting: each frame moves the game on by exactly this many ms (0 = off). */
	private static volatile double stepMillis;
	private static long lastReal = -1;
	private static double game;

	private ReplayClock() {}

	/**
	 * The game timer's reading (ms) for a real one. It never jumps (the game
	 * would rush through the ticks it missed): outside replays it just runs
	 * at real speed from wherever it was.
	 */
	public static synchronized long gameMillis(long realMillis) {
		if (lastReal < 0) {
			lastReal = realMillis;
			game = realMillis;
			return realMillis;
		}
		long delta = Math.max(0, realMillis - lastReal);
		lastReal = realMillis;
		if (!active) {
			game += delta;
		} else if (stepMillis > 0) {
			game += stepMillis;
		} else {
			game += delta * scale;
		}
		return (long) game;
	}

	static void start() {
		active = true;
		scale = 1;
		stepMillis = 0;
	}

	static void stop() {
		active = false;
		scale = 1;
		stepMillis = 0;
	}

	/** Ticks run at this rate (0: frozen; frames still draw). */
	static void scale(double s) {
		scale = s;
	}

	/** Exporting: exact steps per frame (0: back to real time). */
	static void step(double millis) {
		stepMillis = millis;
	}

	public static boolean active() {
		return active;
	}
}
