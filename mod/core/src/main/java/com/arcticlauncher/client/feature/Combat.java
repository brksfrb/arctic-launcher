package com.arcticlauncher.client.feature;

/** Reach and combo from your hits, for the PvP widgets. */
public final class Combat {
	/** Hits further apart than this start a new combo. */
	private static final long COMBO_WINDOW_MS = 2500;
	/** How long the last reach stays on screen. */
	private static final long REACH_SHOWN_MS = 3000;

	private double reach = -1;
	private long lastHit;
	private int combo;
	private int lastHurtTime;

	/** You hit something standing {@code reach} blocks away. */
	public synchronized void attacked(double reach) {
		long now = System.currentTimeMillis();
		combo = now - lastHit <= COMBO_WINDOW_MS ? combo + 1 : 1;
		lastHit = now;
		this.reach = reach;
	}

	/** Each tick: getting hurt breaks the combo. */
	public synchronized void tick(int hurtTime) {
		if (hurtTime > lastHurtTime) {
			combo = 0;
		}
		lastHurtTime = hurtTime;
		if (System.currentTimeMillis() - lastHit > COMBO_WINDOW_MS) {
			combo = 0;
		}
	}

	/** Last reach in blocks, or -1 if it's been a while. */
	public synchronized double reach() {
		return System.currentTimeMillis() - lastHit <= REACH_SHOWN_MS ? reach : -1;
	}

	public synchronized int combo() {
		return combo;
	}
}
