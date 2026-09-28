package com.arcticlauncher.client.hud;

import java.util.ArrayDeque;

/**
 * The server's real tick rate. Servers send their game time about once a
 * second (every 20 ticks); how far it moved against the clock between
 * updates is the TPS. A lagging server shows under 20.
 */
public final class Tps {
	/** Average over this many updates (~5 s). */
	private static final int KEEP = 6;
	/** No update for this long: unknown (paused, left, not sent). */
	private static final long STALE_NS = 5_000_000_000L;
	private static final double MAX_TPS = 20.0;

	/** {arrival nanos, game time}. */
	private static final ArrayDeque<long[]> UPDATES = new ArrayDeque<long[]>();

	private Tps() {}

	/** The server's game time arrived (any thread; repeats are ignored). */
	public static void onServerTime(long gameTime) {
		onServerTime(gameTime, System.nanoTime());
	}

	static synchronized void onServerTime(long gameTime, long now) {
		long[] last = UPDATES.peekLast();
		if (last != null && last[1] == gameTime) {
			return;
		}
		if (last != null && (gameTime < last[1] || now - last[0] > STALE_NS)) {
			// Another world or a long gap: start over.
			UPDATES.clear();
		}
		UPDATES.addLast(new long[] {now, gameTime});
		while (UPDATES.size() > KEEP) {
			UPDATES.removeFirst();
		}
	}

	/** Ticks per second, or -1 when unknown. */
	public static double get() {
		return get(System.nanoTime());
	}

	static synchronized double get(long now) {
		if (UPDATES.size() < 2 || now - UPDATES.peekLast()[0] > STALE_NS) {
			return -1;
		}
		long[] first = UPDATES.peekFirst();
		long[] last = UPDATES.peekLast();
		double seconds = (last[0] - first[0]) / 1e9;
		if (seconds <= 0) {
			return -1;
		}
		return Math.min(MAX_TPS, (last[1] - first[1]) / seconds);
	}

	static synchronized void reset() {
		UPDATES.clear();
	}
}
