package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/** A row with a label, an optional hint, and an on/off switch. */
public class Toggle extends Widget {
	private static final int SWITCH_W = 22;
	private static final int SWITCH_H = 12;
	private static final float SPEED = 8f;

	/** Reads and writes the setting. */
	public interface Binding {
		boolean get();

		void set(boolean on);
	}

	private final String label;
	private final String hint;
	private final Binding binding;
	private float t = -1f;

	public Toggle(String label, String hint, Binding binding) {
		this.label = label;
		this.hint = hint;
		this.binding = binding;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		boolean on = binding.get();
		t = t < 0 ? (on ? 1f : 0f) : approach(t, on ? 1f : 0f, dt * SPEED);
		if (hover > 0f) {
			Draw.round(g, x, y, x + w, y + h, 2, Draw.alpha(s.buttonHover, 0.5f * hover));
		}
		int textRoom = w - SWITCH_W - 12;
		boolean twoLines = hint != null && h >= 22;
		int ty = twoLines ? y + (h - 19) / 2 : y + (h - 8) / 2;
		g.text(Draw.fit(g, label, textRoom), x + 4, ty, s.text, false);
		if (twoLines) {
			g.text(Draw.fit(g, hint, textRoom), x + 4, ty + 11, s.muted, false);
		}
		int sx = x + w - SWITCH_W - 4;
		Skin.toggle(g, s, sx, y + (h - SWITCH_H) / 2, SWITCH_W, SWITCH_H, t, hover > 0.5f);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		binding.set(!binding.get());
		return true;
	}
}
