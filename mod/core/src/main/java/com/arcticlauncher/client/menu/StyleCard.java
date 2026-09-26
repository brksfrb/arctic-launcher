package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/** A clickable preview of a menu style. */
final class StyleCard extends Widget {
	private static final int PREVIEW_H = 44;

	private final Style style;
	private final boolean chosen;
	private final Runnable choose;

	StyleCard(Style style, boolean chosen, Runnable choose) {
		this.style = style;
		this.chosen = chosen;
		this.choose = choose;
	}

	@Override
	protected void draw(Gfx g, Style current, int mx, int my, float dt) {
		Skin.button(g, current, x, y, w, h, true, chosen ? 0f : hover);
		if (chosen) {
			Draw.outline(g, x, y, x + w, y + h, 2, current.accent);
		}
		int px = x + 4;
		int py = y + 4;
		int pw = w - 8;
		preview(g, px, py, pw);
		Draw.centered(g, style.name, x + w / 2, py + PREVIEW_H + 5, chosen ? current.accent : current.text, false);
	}

	/** A tiny menu in this style: sky, mountains and two buttons. */
	private void preview(Gfx g, int px, int py, int pw) {
		g.gradient(px, py, px + pw, py + PREVIEW_H, style.skyTop, style.skyBottom);
		g.fill(px, py + PREVIEW_H - 8, px + pw, py + PREVIEW_H, style.mountainsFront);
		int bw = Math.min(40, pw - 12);
		int bx = px + (pw - bw) / 2;
		Draw.round(g, bx, py + 10, bx + bw, py + 18, 1, style.restyles ? style.accent : 0xFF6F6F6F);
		Draw.round(g, bx, py + 21, bx + bw, py + 29, 1, style.restyles ? style.button : 0xFF4A4A4A);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT || chosen) {
			return false;
		}
		choose.run();
		return true;
	}
}
