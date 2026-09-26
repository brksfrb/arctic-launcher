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

	protected TextWidget(String id, String label, String description, boolean onByDefault) {
		super(id, label, description, onByDefault, Column.LEFT);
		this.label = label;
	}

	protected abstract String value(boolean preview);

	/** Fits its content: label, a small gap, the value. */
	@Override
	public int width(Gfx g) {
		return PAD * 2 + g.textWidth(label) + GAP + g.textWidth(value(false));
	}

	@Override
	public int height() {
		return HEIGHT;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		int w = width(g);
		panel(g, s, 0, 0, w, HEIGHT);
		g.text(label, PAD, 3, s.accent, shadow());
		g.text(value(preview), PAD + g.textWidth(label) + GAP, 3, s.text, shadow());
	}
}
