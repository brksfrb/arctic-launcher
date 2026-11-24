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
		if (s == Style.D4) {
			outlined(g, x, y, x + w, y + h, 2, active ? Draw.mix(s.button, s.buttonHover, hover) : s.buttonOff,
					Draw.mix(s.border, s.accent, active ? hover * 0.55f : 0f));
			return;
		}
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
		if (s == Style.D4) {
			int fill = active ? Draw.mix(s.accent, 0xFF9DE3FF, hover * 0.35f) : s.buttonOff;
			outlined(g, x, y, x + w, y + h, 2, fill, s.border);
			return;
		}
		int fill = active ? Draw.mix(Draw.alpha(s.accent, 0.8f), s.accent, hover) : s.buttonOff;
		Draw.round(g, x, y, x + w, y + h, RADIUS, fill);
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
		int track = Draw.mix(s == Style.D4 ? 0xFF263548 : s.field, s == Style.D4 ? 0xFF319CC6 : Draw.alpha(s.accent, 0.85f), t);
		int radius = Draw.fancy ? Draw.PILL : s == Style.D4 ? 4 : 3;
		Draw.round(g, x, y, x + w, y + h, radius, track);
		Draw.outline(g, x, y, x + w, y + h, radius, hovered ? s.accent : s.border);
		int knob = h - 4;
		int kx = x + 2 + Math.round((w - 4 - knob) * t);
		Draw.round(g, kx, y + 2, kx + knob, y + 2 + knob, Draw.fancy ? Draw.PILL : 2, Draw.mix(s.muted, s == Style.D4 ? s.text : s.onAccent, t));
	}

	/** A translucent panel with a faint border. */
	public static void panel(Gfx g, Style s, int x0, int y0, int x1, int y1) {
		outlined(g, x0, y0, x1, y1, s == Style.D4 ? 2 : 3, s.panel, s.border);
	}
	public static void inset(Gfx g, Style s, int x0, int y0, int x1, int y1) {
		outlined(g, x0, y0, x1, y1, 2, s == Style.D4 ? 0xD20B1422 : s.field, s.border);
	}

	public static void outlined(Gfx g, int x0, int y0, int x1, int y1, int radius, int fill, int stroke) {
		Draw.round(g, x0, y0, x1, y1, radius, fill);
		Draw.outline(g, x0, y0, x1, y1, radius, stroke);
	}
}
