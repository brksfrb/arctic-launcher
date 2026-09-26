package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.style.Style;

/**
 * The animated Arctic night behind menus: sky, aurora, stars, moon,
 * mountains and falling snow, matching the launcher. Everything is
 * computed from the clock, so there's no state to keep.
 */
public final class Backdrop {
	private static final int STARS = 60;
	private static final int FLAKES = 80;
	private static final int AURORA_STEP = 3;
	private static final int MOUNTAIN_STEP = 2;

	private Backdrop() {}

	public static void render(Gfx g, Style s) {
		int w = g.width();
		int h = g.height();
		double t = (System.currentTimeMillis() % 3600000L) / 1000.0;
		g.gradient(0, 0, w, h, s.skyTop, s.skyBottom);
		stars(g, w, h, t);
		moon(g, s, (int) (w * 0.82), (int) (h * 0.17), Math.max(6, h / 26), h);
		aurora(g, s, w, h, t);
		mountains(g, w, h, 0.66, 0.20, 7, 3, s.mountainsBack);
		mountains(g, w, h, 0.80, 0.16, 11, 5, s.mountainsFront);
		snow(g, w, h, t);
	}

	/** A crescent: a disc with a sky-colored disc over one side. */
	private static void moon(Gfx g, Style s, int x, int y, int r, int h) {
		Draw.disc(g, x, y, r, 0xFFEFF6FF);
		int sky = Draw.mix(s.skyTop, s.skyBottom, (y - 2) / (float) h);
		Draw.disc(g, x + Math.max(3, r / 2), y - 2, r, sky);
	}

	/** A deterministic pseudo-random number in [0, 1). */
	private static double rand(int seed) {
		double v = Math.sin(seed * 12.9898 + 78.233) * 43758.5453;
		return v - Math.floor(v);
	}

	private static void stars(Gfx g, int w, int h, double t) {
		for (int i = 0; i < STARS; i++) {
			int x = (int) (rand(i) * w);
			int y = (int) (rand(i + 500) * h * 0.55);
			float twinkle = (float) (0.35 + 0.35 * Math.sin(t * (0.6 + rand(i + 900)) + i));
			g.fill(x, y, x + 1, y + 1, Draw.alpha(0xFFFFFFFF, twinkle));
		}
	}

	private static void aurora(Gfx g, Style s, int w, int h, double t) {
		int clear = s.aurora & 0x00FFFFFF;
		int band = Math.max(8, (int) (h * 0.12));
		for (int x = 0; x < w; x += AURORA_STEP) {
			double wave = Math.sin(x * 0.018 + t * 0.25) * 0.6 + Math.sin(x * 0.043 - t * 0.17) * 0.4;
			int cy = (int) (h * 0.24 + wave * h * 0.06);
			float strength = (float) (0.45 + 0.55 * Math.sin(x * 0.011 + t * 0.13));
			int c = Draw.alpha(s.aurora, strength);
			g.gradient(x, cy - band, x + AURORA_STEP, cy, clear, c);
			g.gradient(x, cy, x + AURORA_STEP, cy + band / 3, c, clear);
		}
	}

	/** A ridge of peaks from {@code base} (fraction of height) up by {@code height}. */
	private static void mountains(Gfx g, int w, int h, double base, double height, int peaks, int seed, int color) {
		int baseY = (int) (h * base);
		for (int x = 0; x < w; x += MOUNTAIN_STEP) {
			double top = 0;
			for (int i = 0; i < peaks; i++) {
				double px = (i + rand(seed * 31 + i)) / peaks * w;
				double pw = w / (double) peaks * (0.7 + rand(seed * 17 + i) * 0.8);
				double ph = 0.45 + rand(seed * 7 + i) * 0.55;
				top = Math.max(top, ph * (1 - Math.abs(x - px) / pw));
			}
			int y = baseY - (int) (top * h * height);
			g.fill(x, y, x + MOUNTAIN_STEP, h, color);
		}
	}

	private static void snow(Gfx g, int w, int h, double t) {
		for (int i = 0; i < FLAKES; i++) {
			double speed = 6 + rand(i + 100) * 14;
			double fall = rand(i + 200) * (h + 10) + t * speed;
			int y = (int) (fall % (h + 10)) - 5;
			int x = (int) (rand(i + 300) * w + Math.sin(t * 0.7 + i) * 6);
			int size = i % 4 == 0 ? 2 : 1;
			float a = (float) (0.35 + rand(i + 400) * 0.5);
			g.fill(x, y, x + size, y + size, Draw.alpha(0xFFFFFFFF, a));
		}
	}
}
