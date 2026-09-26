package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/** A labelled button; {@link #primary()} makes it the accent-filled one. */
public class Button extends Widget {
	private final String label;
	private final Runnable action;
	private boolean primary;
	private boolean selected;

	public Button(String label, Runnable action) {
		this.label = label;
		this.action = action;
	}

	public Button primary() {
		primary = true;
		return this;
	}

	/** Draw as the chosen one of a group (tabs). */
	public Button selected(boolean on) {
		selected = on;
		return this;
	}

	public Button enabled(boolean on) {
		enabled = on;
		return this;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int textColor;
		if (primary || selected) {
			Skin.primary(g, s, x, y, w, h, enabled, hover);
			textColor = enabled ? s.onAccent : s.muted;
		} else {
			Skin.button(g, s, x, y, w, h, enabled, hover);
			textColor = enabled ? s.text : s.muted;
		}
		String text = Draw.fit(g, label, w - 6);
		Draw.centered(g, text, x + w / 2, y + (h - 8) / 2, textColor, !(primary || selected));
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		action.run();
		return true;
	}
}
