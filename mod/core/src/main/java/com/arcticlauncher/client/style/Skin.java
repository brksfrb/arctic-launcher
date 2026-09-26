package com.arcticlauncher.client.style;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * How widgets look in a style. Arctic's own widgets and the restyled vanilla
 * ones both draw through here, so every screen matches.
 */
public final class Skin {
	private static final int RADIUS = 2;

	private Skin() {}

	public static void button(Gfx g, Style s, int x, int y, int w, int h, boolean active, float hover) {
		if (!active) {
			Draw.round(g, x, y, x + w, y + h, RADIUS, s.buttonOff);
			return;
		}
		Draw.round(g, x, y, x + w, y + h, RADIUS, Draw.mix(s.button, s.buttonHover, hover));
		if (hover > 0f) {
			Draw.outline(g, x, y, x + w, y + h, RADIUS, Draw.alpha(s.accent, 0.35f + 0.45f * hover));
		}
		// A soft accent underline marks buttons as clickable.
		g.fill(x + 3, y + h - 1, x + w - 3, y + h, Draw.alpha(s.accent, 0.25f + 0.5f * hover));
	}

	/** A filled accent button (the main action). */
	public static void primary(Gfx g, Style s, int x, int y, int w, int h, boolean active, float hover) {
		int base = active ? Draw.mix(Draw.alpha(s.accent, 0.8f), s.accent, hover) : s.buttonOff;
		Draw.round(g, x, y, x + w, y + h, RADIUS, base);
	}

	public static void sliderTrack(Gfx g, Style s, int x, int y, int w, int h, boolean hovered) {
		Draw.round(g, x, y, x + w, y + h, RADIUS, s.field);
		Draw.outline(g, x, y, x + w, y + h, RADIUS, hovered ? Draw.alpha(s.accent, 0.6f) : s.border);
	}

	public static void sliderHandle(Gfx g, Style s, int x, int y, int w, int h, boolean hovered) {
		Draw.round(g, x + 1, y + 1, x + w - 1, y + h - 1, RADIUS, hovered ? s.accent : Draw.alpha(s.accent, 0.8f));
	}

	public static void field(Gfx g, Style s, int x, int y, int w, int h, boolean focused) {
		Draw.round(g, x, y, x + w, y + h, RADIUS, s.field);
		Draw.outline(g, x, y, x + w, y + h, RADIUS, focused ? s.accent : s.border);
	}

	public static void checkbox(Gfx g, Style s, int x, int y, int size, boolean selected, boolean hovered) {
		if (selected) {
			Draw.round(g, x, y, x + size, y + size, RADIUS, s.accent);
			check(g, x, y, size, s.onAccent);
		} else {
			Draw.round(g, x, y, x + size, y + size, RADIUS, s.field);
		}
		Draw.outline(g, x, y, x + size, y + size, RADIUS, hovered ? s.accent : s.border);
	}

	/** A pixel check mark inside a square. */
	private static void check(Gfx g, int x, int y, int size, int color) {
		int u = Math.max(1, size / 9);
		int ox = x + (size - 9 * u) / 2;
		int oy = y + (size - 9 * u) / 2;
		for (int[] c : CHECK) {
			g.fill(ox + c[0] * u, oy + c[1] * u, ox + (c[0] + 1) * u, oy + (c[1] + 2) * u, color);
		}
	}

	/** Check mark cells on a 9x9 grid (each two cells tall). */
	private static final int[][] CHECK = {{1, 4}, {2, 5}, {3, 6}, {4, 5}, {5, 4}, {6, 3}, {7, 2}};

	/** An on/off switch; {@code t} animates from off (0) to on (1). */
	public static void toggle(Gfx g, Style s, int x, int y, int w, int h, float t, boolean hovered) {
		int track = Draw.mix(s.field, Draw.alpha(s.accent, 0.85f), t);
		Draw.round(g, x, y, x + w, y + h, 3, track);
		Draw.outline(g, x, y, x + w, y + h, 3, hovered ? s.accent : s.border);
		int knob = h - 4;
		int kx = x + 2 + Math.round((w - 4 - knob) * t);
		Draw.round(g, kx, y + 2, kx + knob, y + 2 + knob, 2, t > 0.5f ? s.onAccent : s.muted);
	}

	/** A translucent panel with a faint border. */
	public static void panel(Gfx g, Style s, int x0, int y0, int x1, int y1) {
		Draw.round(g, x0, y0, x1, y1, 3, s.panel);
		Draw.outline(g, x0, y0, x1, y1, 3, s.border);
	}
}
