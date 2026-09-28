package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.style.Style;

/** One HUD element (FPS, keystrokes, …), drawn at its own top-left. */
public abstract class HudWidget {
	/** Which side a widget stacks on until the player moves it. */
	public enum Column {
		LEFT,
		RIGHT
	}

	public final String id;
	public final String name;
	public final String description;
	final boolean onByDefault;
	final Column column;
	/** How the player set this widget to look (colors, box, shadow…). */
	protected HudSlot look = new HudSlot(true);
	/** One chroma cycle, in milliseconds. */
	private static final float CHROMA_MS = 4000f;
	private static final float MUTED = 0.7f;

	protected HudWidget(String id, String name, String description, boolean onByDefault, Column column) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.onByDefault = onByDefault;
		this.column = column;
	}

	/** Draw with these settings from now on (the HUD sets it before each draw). */
	public void use(HudSlot slot) {
		look = slot;
	}

	/** The widget's box, unless the player turned it off. */
	protected void panel(Gfx g, Style s, int x0, int y0, int x1, int y1) {
		if (!look.background) {
			return;
		}
		if (look.radius > 0) {
			com.arcticlauncher.client.gfx.Draw.round(g, x0, y0, x1, y1, look.radius, look.backgroundColor);
		} else {
			g.fill(x0, y0, x1, y1, look.backgroundColor);
		}
		if (look.border) {
			com.arcticlauncher.client.gfx.Draw.outline(g, x0, y0, x1, y1, Math.max(1, look.radius), look.borderColor);
		}
	}

	protected boolean background() {
		return look.background;
	}

	protected boolean shadow() {
		return look.shadow;
	}

	/** The main text color (cycling through the rainbow with chroma on). */
	protected int text() {
		return look.chroma ? chroma(0) : look.textColor;
	}

	/** Labels and highlights: their own color, or the text's. */
	protected int accent() {
		return look.labelColor != 0 && !look.chroma ? look.labelColor : text();
	}

	/** Secondary text (times, maxima). */
	protected int muted() {
		return com.arcticlauncher.client.gfx.Draw.alpha(text(), MUTED);
	}

	/** A rainbow color that moves with time; {@code offset} shifts it (pixels across). */
	protected static int chroma(int offset) {
		float hue = ((System.currentTimeMillis() % (long) CHROMA_MS) / CHROMA_MS + offset / 300f) % 1f;
		return 0xFF000000 | hsv(hue, 0.55f, 1f);
	}

	private static int hsv(float h, float s, float v) {
		int i = (int) (h * 6) % 6;
		float f = h * 6 - (int) (h * 6);
		float p = v * (1 - s);
		float q = v * (1 - f * s);
		float t = v * (1 - (1 - f) * s);
		float[][] rgb = {{v, t, p}, {q, v, p}, {p, v, t}, {p, q, v}, {t, p, v}, {v, p, q}};
		float[] c = rgb[i];
		return (Math.round(c[0] * 255) << 16) | (Math.round(c[1] * 255) << 8) | Math.round(c[2] * 255);
	}

	/** Needs the version's game hooks (hidden where they don't exist). */
	public boolean needsGame() {
		return false;
	}

	public abstract int width(Gfx g);

	public abstract int height();

	/**
	 * Draw at (0, 0). {@code preview} is true in the layout editor, where
	 * widgets show sample values when there's nothing live to show.
	 */
	public abstract void render(Gfx g, Style s, boolean preview);
}
