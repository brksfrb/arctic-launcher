package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/** A color to pick; the chosen one gets an accent ring. */
final class Swatch extends Widget {
	private final int color;
	private final boolean chosen;
	private final Runnable pick;

	Swatch(int color, boolean chosen, Runnable pick) {
		this.color = color;
		this.chosen = chosen;
		this.pick = pick;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		Draw.round(g, x, y, x + w, y + h, 2, color);
		int ring = chosen ? s.accent : hover > 0.5f ? s.text : s.border;
		Draw.outline(g, x - 1, y - 1, x + w + 1, y + h + 1, 2, ring);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		pick.run();
		return true;
	}
}
