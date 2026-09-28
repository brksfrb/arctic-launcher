package com.arcticlauncher.client.feature;

/**
 * A stopwatch on one key: press to start, press to stop, press again to
 * clear (the next press starts over).
 */
public final class Stopwatch {
	private long startedAt;
	private long elapsed;
	private boolean running;
	private boolean wasDown;

	/** Each tick with the key's state. */
	public synchronized void tick(boolean down) {
		if (down && !wasDown) {
			press(System.currentTimeMillis());
		}
		wasDown = down;
	}

	synchronized void press(long now) {
		if (running) {
			elapsed += now - startedAt;
			running = false;
		} else if (elapsed > 0) {
			elapsed = 0;
		} else {
			startedAt = now;
			running = true;
		}
	}

	/** Milliseconds on the clock. */
	public synchronized long millis() {
		return millis(System.currentTimeMillis());
	}

	synchronized long millis(long now) {
		return elapsed + (running ? now - startedAt : 0);
	}

	public synchronized boolean running() {
		return running;
	}

	/** "1:05.3" (or "1:02:05" past an hour). */
	public static String format(long ms) {
		long tenths = ms / 100 % 10;
		long seconds = ms / 1000;
		if (seconds >= 3600) {
			return String.format(java.util.Locale.ROOT, "%d:%02d:%02d", seconds / 3600, seconds / 60 % 60, seconds % 60);
		}
		return String.format(java.util.Locale.ROOT, "%d:%02d.%d", seconds / 60, seconds % 60, tenths);
	}
}
