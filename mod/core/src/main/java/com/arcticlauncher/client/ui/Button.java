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
	private static final long CONFIRM_NS = 3_000_000_000L;
	private static final int DANGER = 0xFFF87171;

	private boolean primary;
	private boolean selected;
	/** When set, the first click shows this and only a second click acts. */
	private String confirm;
	private long armedUntil;

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

	/** Ask before acting: the first click shows {@code prompt} for a few seconds. */
	public Button confirm(String prompt) {
		confirm = prompt;
		return this;
	}

	private boolean armed() {
		return confirm != null && System.nanoTime() < armedUntil;
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
		boolean armed = armed();
		if (armed) {
			Draw.outline(g, x, y, x + w, y + h, 2, DANGER);
			textColor = DANGER;
		}
		String text = Draw.fitCentered(g, armed ? confirm : label, w - 6, x + w / 2, y + (h - 8) / 2);
		Draw.centered(g, text, x + w / 2, y + (h - 8) / 2, textColor, !(primary || selected));
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		if (confirm != null && !armed()) {
			armedUntil = System.nanoTime() + CONFIRM_NS;
			return true;
		}
		armedUntil = 0;
		action.run();
		return true;
	}
}
