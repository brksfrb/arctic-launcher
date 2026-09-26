package com.arcticlauncher.client.hud;

/** Clicks per second for the left and right mouse buttons. */
public final class Cps {
	private static final long WINDOW_MS = 1000;
	private static final int MAX = 64;

	/** Ring buffers of click times per button. */
	private final long[][] times = new long[2][MAX];
	private final int[] next = new int[2];

	public synchronized void click(int button) {
		if (button < 0 || button > 1) {
			return;
		}
		times[button][next[button]] = System.currentTimeMillis();
		next[button] = (next[button] + 1) % MAX;
	}

	public synchronized int get(int button) {
		long since = System.currentTimeMillis() - WINDOW_MS;
		int n = 0;
		for (long t : times[button]) {
			if (t > since) {
				n++;
			}
		}
		return n;
	}
}
