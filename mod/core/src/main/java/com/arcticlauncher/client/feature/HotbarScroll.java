package com.arcticlauncher.client.feature;

/**
 * Where the hotbar selection goes after a scroll. Slots are 0–8, shown as 1–9.
 * Vanilla wraps (slot 9 then slot 1). With wrapping off, slot 1 and slot 9 stay put.
 */
public final class HotbarScroll {
	public static final int SLOTS = 9;

	private HotbarScroll() {}

	/**
	 * Move {@code step} slots toward slot 9 (positive) or slot 1 (negative).
	 * Wrapping uses the same one-step cycle as the game ({@code floorMod} over 9).
	 */
	public static int move(int current, int step, boolean wrap) {
		int next = current + step;
		if (wrap) {
			return Math.floorMod(next, SLOTS);
		}
		if (next < 0) {
			return 0;
		}
		if (next >= SLOTS) {
			return SLOTS - 1;
		}
		return next;
	}

	/**
	 * One notch of the scroll wheel, the way the game reads it: the sign only (so a big
	 * delta is still one slot), and a positive wheel moves toward slot 1.
	 */
	public static int wheel(int current, double wheel, boolean wrap) {
		return move(current, -((int) Math.signum(wheel)), wrap);
	}
}
