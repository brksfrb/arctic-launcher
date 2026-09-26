package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** A label and a value in a small pill: "FPS 144". */
public abstract class TextWidget extends HudWidget {
	private static final int PAD = 4;
	private static final int GAP = 4;
	private static final int HEIGHT = 13;

	private final String label;
	/** The widest value expected, so the pill doesn't jitter. */
	private final String sample;

	protected TextWidget(String id, String label, String description, String sample, boolean onByDefault) {
		super(id, label, description, onByDefault, Column.LEFT);
		this.label = label;
		this.sample = sample;
	}

	protected abstract String value(boolean preview);

	/** Fits the widest expected value, and grows if a value is wider (FPS 1000+). */
	@Override
	public int width(Gfx g) {
		int value = Math.max(g.textWidth(sample), g.textWidth(value(false)));
		return PAD * 2 + g.textWidth(label) + GAP + value;
	}

	@Override
	public int height() {
		return HEIGHT;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		int w = width(g);
		Draw.round(g, 0, 0, w, HEIGHT, 2, s.hud);
		g.text(label, PAD, 3, s.accent, false);
		String v = value(preview);
		g.text(v, w - PAD - g.textWidth(v), 3, s.text, false);
	}
}
