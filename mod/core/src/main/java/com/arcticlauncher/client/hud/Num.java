package com.arcticlauncher.client.hud;

/**
 * Numbers for HUD text, drawn every frame: {@code String.format("%.1f")}
 * builds a formatter and parses its pattern on every call, which showed up
 * as half of a busy HUD's work.
 */
final class Num {
	private static final long[] SCALE = {1, 10, 100, 1000};

	private Num() {}

	/** {@code value} with {@code decimals} (0-3) digits after the point, rounded half up, as "%.Nf" in Locale.ROOT. */
	static String fixed(double value, int decimals) {
		if (Double.isNaN(value) || Double.isInfinite(value) || Math.abs(value) > 1e15) {
			return String.format(java.util.Locale.ROOT, "%." + decimals + "f", value);
		}
		long scale = SCALE[decimals];
		long rounded = (long) Math.floor(Math.abs(value) * scale + 0.5);
		StringBuilder out = new StringBuilder(12);
		if (value < 0 && rounded != 0) {
			out.append('-');
		}
		out.append(rounded / scale);
		if (decimals > 0) {
			out.append('.');
			String fraction = Long.toString(rounded % scale);
			for (int i = fraction.length(); i < decimals; i++) {
				out.append('0');
			}
			out.append(fraction);
		}
		return out.toString();
	}
}
