package com.arcticlauncher.client.menu;

import java.util.function.Supplier;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Hints;
import com.arcticlauncher.client.ui.Widget;

/**
 * A pack in a list: its icon (or its first letter until the icon is in),
 * the name, and a line under it. Buttons sit on top of it, on the right.
 */
final class PackRow extends Widget {
	static final int H = 32;
	private static final int ICON = 24;

	private final String title;
	private final String line;
	private final Supplier<String> icon;
	/** Room kept free on the right for the row's buttons. */
	private final int buttons;
	private final boolean dim;

	PackRow(String title, String line, Supplier<String> icon, int buttons, boolean dim) {
		this.title = title;
		this.line = line;
		this.icon = icon;
		this.buttons = buttons;
		this.dim = dim;
		enabled = false;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int iy = y + (h - ICON) / 2;
		String key = icon.get();
		if (key != null) {
			g.texture(key, x + 2, iy, ICON, ICON, 0, 0, 64, 64, 64, 64);
		} else {
			Draw.round(g, x + 2, iy, x + 2 + ICON, iy + ICON, 3, Draw.alpha(s.accent, 0.25f));
			String letter = title.isEmpty() ? "?" : title.substring(0, 1).toUpperCase(java.util.Locale.ROOT);
			Draw.centered(g, letter, x + 2 + ICON / 2, iy + (ICON - 8) / 2, s.text, false);
		}
		int tx = x + ICON + 8;
		int room = w - (tx - x) - buttons - 6;
		int color = dim ? s.muted : s.text;
		g.text(Draw.fit(g, title, room, tx, y + 5), tx, y + 5, color, false);
		String shown = Draw.fit(g, line, room, tx, y + 17);
		g.text(shown, tx, y + 17, s.muted, false);
		if (!shown.equals(line)) {
			Hints.offer(line, tx, y + 15, room, 11);
		}
	}
}
