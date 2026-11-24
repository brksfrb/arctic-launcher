package com.arcticlauncher.client.replay;

/** Converts replay time committed by the packet source into the game's monotonic tick time. */
public final class ReplayClock {
	private static volatile boolean active;
	private static long lastReal = -1;
	private static double game;
	private static double pending;

	private ReplayClock() {}

	public static synchronized long gameMillis(long realMillis) {
		if (lastReal < 0) {
			lastReal = realMillis;
			game = realMillis;
		}
		game += active ? pending : Math.max(0, realMillis - lastReal);
		pending = 0;
		lastReal = realMillis;
		return (long) game;
	}

	static synchronized void start() {
		pending = 0;
		active = true;
	}

	static synchronized void stop() {
		pending = 0;
		active = false;
	}

	static synchronized void advance(double millis) {
		pending += millis;
	}

	public static boolean active() {
		return active;
	}
}
