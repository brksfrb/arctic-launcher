package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/**
 * Any color: a square for saturation (left to right) and brightness (top to bottom) of the chosen
 * hue, and a strip under it for the hue. The color is kept as hue, saturation and brightness in
 * {@code hsv}, so a gray or a black doesn't lose the hue while it's dragged.
 */
final class ColorPicker extends Widget {
	static final int SQUARE_H = 56;
	static final int HUE_H = 10;
	private static final int GAP = 4;
	static final int HEIGHT = SQUARE_H + GAP + HUE_H;
	/** Width of one drawn column: the gradient is a row of these. */
	private static final int STEP = 2;

	private static final int NONE = 0;
	private static final int SQUARE = 1;
	private static final int HUE = 2;

	private final float[] hsv;
	/** Runs while dragging (the color changed). */
	private final Runnable changed;
	/** Runs when the mouse is let go (save it). */
	private final Runnable finished;
	private int dragging = NONE;

	ColorPicker(float[] hsv, Runnable changed, Runnable finished) {
		this.hsv = hsv;
		this.changed = changed;
		this.finished = finished;
	}

	/** Opaque ARGB from hue, saturation and brightness (all 0 to 1). */
	static int rgb(float h, float s, float v) {
		float hh = (h - (float) Math.floor(h)) * 6f;
		int sector = (int) hh;
		float f = hh - sector;
		float p = v * (1f - s);
		float q = v * (1f - s * f);
		float t = v * (1f - s * (1f - f));
		float r;
		float g;
		float b;
		switch (sector) {
			case 0: r = v; g = t; b = p; break;
			case 1: r = q; g = v; b = p; break;
			case 2: r = p; g = v; b = t; break;
			case 3: r = p; g = q; b = v; break;
			case 4: r = t; g = p; b = v; break;
			default: r = v; g = p; b = q; break;
		}
		return 0xFF000000 | Math.round(r * 255) << 16 | Math.round(g * 255) << 8 | Math.round(b * 255);
	}

	/** Hue, saturation and brightness of an RGB color, into {@code out}. */
	static void toHsv(int argb, float[] out) {
		float r = ((argb >> 16) & 0xFF) / 255f;
		float g = ((argb >> 8) & 0xFF) / 255f;
		float b = (argb & 0xFF) / 255f;
		float max = Math.max(r, Math.max(g, b));
		float min = Math.min(r, Math.min(g, b));
		float d = max - min;
		float h;
		if (d == 0) {
			// A gray: keep the hue it had.
			h = out[0];
		} else if (max == r) {
			h = ((g - b) / d + 6f) % 6f / 6f;
		} else if (max == g) {
			h = ((b - r) / d + 2f) / 6f;
		} else {
			h = ((r - g) / d + 4f) / 6f;
		}
		out[0] = h;
		out[1] = max == 0 ? 0 : d / max;
		out[2] = max;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int right = x + w;
		int span = Math.max(1, w - 1);
		for (int cx = x; cx < right; cx += STEP) {
			int end = Math.min(right, cx + STEP);
			float sat = (cx - x) / (float) span;
			g.gradient(cx, y, end, y + SQUARE_H, rgb(hsv[0], sat, 1f), rgb(hsv[0], sat, 0f));
		}
		int hy = y + SQUARE_H + GAP;
		for (int cx = x; cx < right; cx += STEP) {
			int end = Math.min(right, cx + STEP);
			g.fill(cx, hy, end, hy + HUE_H, rgb((cx - x) / (float) span, 1f, 1f));
		}
		int px = x + Math.round(hsv[1] * span);
		int py = y + Math.round((1f - hsv[2]) * (SQUARE_H - 1));
		Draw.outline(g, px - 3, py - 3, px + 4, py + 4, 1, 0xFF000000);
		Draw.outline(g, px - 2, py - 2, px + 3, py + 3, 1, 0xFFFFFFFF);
		int hx = x + Math.round(hsv[0] * span);
		g.fill(hx - 2, hy - 2, hx + 3, hy + HUE_H + 2, 0xFF000000);
		g.fill(hx - 1, hy - 1, hx + 2, hy + HUE_H + 1, 0xFFFFFFFF);
		Draw.outline(g, x - 1, y - 1, right + 1, y + SQUARE_H + 1, 2, s.border);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		dragging = my < y + SQUARE_H + GAP / 2 ? SQUARE : HUE;
		apply(mx, my);
		return true;
	}

	@Override
	public boolean drag(double mx, double my, int button) {
		if (dragging == NONE || button != Keys.MOUSE_LEFT) {
			return false;
		}
		apply(mx, my);
		return true;
	}

	@Override
	public void release(double mx, double my, int button) {
		if (dragging != NONE) {
			dragging = NONE;
			finished.run();
		}
	}

	private void apply(double mx, double my) {
		float across = clamp((float) (mx - x) / Math.max(1, w - 1));
		if (dragging == SQUARE) {
			hsv[1] = across;
			hsv[2] = 1f - clamp((float) (my - y) / (SQUARE_H - 1));
		} else {
			hsv[0] = across;
		}
		changed.run();
	}

	private static float clamp(float v) {
		return Math.max(0f, Math.min(1f, v));
	}
}
