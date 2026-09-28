package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/**
 * A mod on the Mods screen: its icon and name, and a bar at the bottom that
 * switches it on and off (green when on). Clicking the rest of the card opens
 * its settings; mods without a switch open on any click.
 */
final class ModCard extends Widget {
	static final int BAR = 16;
	private static final int ICON = 32;
	private static final int ON = 0xFF22C55E;
	private static final int OFF = 0xFF3F3F46;
	private static final int GEAR = 12;

	private final Mod mod;
	private final Runnable open;
	private final Runnable changed;

	ModCard(Mod mod, Runnable open, Runnable changed) {
		this.mod = mod;
		this.open = open;
		this.changed = changed;
	}

	private boolean onBar(double my) {
		return my >= y + h - BAR;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		Skin.button(g, s, x, y, w, h, true, hover * 0.6f);
		int iconY = y + (h - BAR - ICON - 12) / 2;
		g.texture(mod.iconKey(), x + (w - ICON) / 2, iconY, ICON, ICON, 0, 0, 32, 32, 32, 32);
		String name = Draw.fitCentered(g, mod.name, w - 6, x + w / 2, iconY + ICON + 4);
		Draw.centered(g, name, x + w / 2, iconY + ICON + 4, s.text, true);
		int by = y + h - BAR;
		boolean overBar = hover > 0f && onBar(my) && contains(mx, my);
		if (mod.hasSwitch()) {
			boolean on = mod.on();
			int color = on ? ON : OFF;
			Draw.round(g, x + 2, by, x + w - 2, y + h - 2, 2, overBar ? Draw.mix(color, 0xFFFFFFFF, 0.15f) : color);
			Draw.centered(g, on ? "ON" : "OFF", x + w / 2, by + (BAR - 2 - 8) / 2 + 1, 0xFFFFFFFF, true);
		} else {
			Draw.round(g, x + 2, by, x + w - 2, y + h - 2, 2, Draw.alpha(s.accent, overBar ? 0.5f : 0.3f));
			Draw.centered(g, "Open", x + w / 2, by + (BAR - 2 - 8) / 2 + 1, s.text, true);
		}
		com.arcticlauncher.client.ui.Hints.offer(mod.description, x, y, w, h - BAR);
		if (mod.hasSettings() && mod.hasSwitch()) {
			g.texture("asset:icons/settings", x + w - GEAR - 4, y + 4, GEAR, GEAR, 0, 0, 32, 32, 32, 32);
		}
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		if (mod.hasSwitch() && (onBar(my) || !mod.hasSettings())) {
			mod.setOn(!mod.on());
			changed.run();
			return true;
		}
		open.run();
		return true;
	}
}
