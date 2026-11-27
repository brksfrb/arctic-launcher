package com.arcticlauncher.legacy;

/**
 * Converts an LWJGL2 wheel event into the scrollY units modern Minecraft passes
 * into screen handlers.
 *
 * <p>A single 1.8.9 event is otherwise clamped to one notch, and one vanilla notch
 * only moves half a row (20px in Arctic lists). That feels far slower than modern
 * versions, where a flick keeps its magnitude and lists feel quicker. One notch
 * here is two modern scrollY units: 40px in Arctic lists, one full vanilla row.
 *
 * <p>LWJGL2 reports 120 units per notch on Windows (and can combine notches in one
 * event) but often only ±1 on macOS and Linux. Never divide those small values by
 * 120, or scrolling becomes almost still.
 */
public final class LegacyScroll {
	/** One physical notch, in modern screen scrollY units. */
	public static final double NOTCH = 2;

	private LegacyScroll() {}

	public static double amount(int wheel) {
		if (wheel == 0) {
			return 0;
		}
		double notches = Math.abs(wheel) >= 120 ? wheel / 120.0 : (wheel > 0 ? 1.0 : -1.0);
		return notches * NOTCH;
	}
}
