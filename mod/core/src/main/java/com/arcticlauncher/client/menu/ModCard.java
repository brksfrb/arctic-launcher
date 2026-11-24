package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/** Card body selects the feature, the gear opens its full settings and the footer switches it. */
final class ModCard extends Widget {
	private static final int BAR = 12;
	private static final int GEAR = 9;

	private final Mod mod;
	private final boolean selected;
	private final Runnable select;
	private final Runnable open;
	private final String onLabel = Lang.t("mods.badge_on");
	private final String offLabel = Lang.t("mods.badge_off");
	private final String openLabel = Lang.t("mods.open");
	private float toggle = -1f;

	ModCard(Mod mod, boolean selected, Runnable select, Runnable open) {
		this.mod = mod;
		this.selected = selected;
		this.select = select;
		this.open = open;
	}

	private boolean onBar(double my) {
		return my >= y + h - BAR - 2;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int base = s == Style.D4 ? 0xA8121C2B : s.button;
		int fill = Draw.mix(base, s.buttonHover, selected ? 0.65f : hover * 0.45f);
		int stroke = Draw.mix(s.border, s.accent, selected ? 0.72f : hover * 0.25f);
		Skin.outlined(g, x, y, x + w, y + h, 2, fill, stroke);
		int icon = h <= 63 ? 22 : 26;
		int iconY = y + 8;
		g.texture(mod.iconKey(), x + (w - icon) / 2, iconY, icon, icon, 0, 0, 32, 32, 32, 32);
		int center = x + w / 2;
		int titleY = iconY + icon + 3;
		Draw.centered(g, Draw.fitCentered(g, mod.name, w - 8, center, titleY), center, titleY, s.text, false);
		int by = y + h - BAR - 2;
		boolean on = mod.on();
		toggle = toggle < 0 ? (on ? 1f : 0f) : approach(toggle, on ? 1f : 0f, dt * 9f);
		float over = onBar(my) && contains(mx, my) ? hover * 0.2f : 0f;
		int barFill = Draw.mix(Draw.alpha(s.field, 0.55f), Draw.alpha(s.accent, 0.25f), toggle + (1f - toggle) * over);
		int barStroke = Draw.mix(Draw.alpha(s.muted, 0.48f), Draw.alpha(s.accent, 0.82f), toggle);
		Skin.outlined(g, x + 3, by, x + w - 3, y + h - 2, 1, barFill, barStroke);
		Draw.centered(g, mod.hasSwitch() ? (on ? onLabel : offLabel) : openLabel, center, by + 2,
				mod.hasSwitch() ? Draw.mix(s.muted, s.accent, toggle) : s.text, false);
		if (mod.hasSettings()) {
			g.texture("asset:icons/settings", x + w - GEAR - 4, y + 4, GEAR, GEAR, 0, 0, 32, 32, 32, 32);
		}
		com.arcticlauncher.client.ui.Hints.offer(mod.description, x, y, w, h - BAR);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) return false;
		ModCatalog.markSeen(mod.id);
		if (onBar(my) && mod.hasSwitch()) {
			mod.setOn(!mod.on());
		} else if (mod.hasSettings() && (my < y + 16 && mx >= x + w - 17 || onBar(my))) {
			open.run();
		} else if (mod.hasSettings() || mod.hasSwitch()) {
			select.run();
		}
		return true;
	}
}
