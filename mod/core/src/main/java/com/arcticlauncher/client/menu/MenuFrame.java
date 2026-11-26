package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.gfx.BrandMark;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.ui.Widget;

/** Shared frame and navigation for the catalog and full settings. */
final class MenuFrame {
	private static final Mod.Category[] CATEGORIES = {
			Mod.Category.ALL, Mod.Category.HUD, Mod.Category.VISUAL, Mod.Category.GAMEPLAY,
			Mod.Category.CHAT, Mod.Category.SOCIAL, Mod.Category.CLIENT
	};
	private static final String[] ICONS = {
			"asset:icons/mods", "asset:icons/hud", "asset:icons/light", "asset:icons/movement",
			"asset:icons/chat", "asset:icons/friends", "asset:icons/settings"
	};
	final int x, y, right, bottom, content, top;
	private final int divider;
	private final boolean sidebar;

	MenuFrame(int width, int height) {
		x = width >= 450 ? 14 : 5;
		y = height >= 260 ? 15 : 5;
		right = Math.max(x + 1, width - x);
		bottom = Math.max(y + 1, height - y);
		sidebar = width >= 400 && height >= 200;
		divider = x + 114;
		content = sidebar ? divider + 8 : x + 6;
		top = y + 7;
	}

	int end() {
		return right - 8;
	}

	int navigation(Host host, Mod.Category selected, java.util.function.Consumer<Mod.Category> change, Runnable openAccount) {
		if (!sidebar) {
			int cell = Math.max(1, (end() - content) / CATEGORIES.length);
			for (int i = 0; i < CATEGORIES.length; i++) {
				final Mod.Category category = CATEGORIES[i];
				host.add(new com.arcticlauncher.client.ui.Button(category.title(), () -> change.accept(category))
						.selected(category == selected)).bounds(content + i * cell, top + 24, cell - 1, 16);
			}
			return top + 44;
		}
		for (int i = 0; i < CATEGORIES.length; i++) {
			int index = i;
			Mod.Category category = CATEGORIES[index];
			host.add(new Widget() {
				@Override
				protected void draw(Gfx g, Style s, int mx, int my, float dt) {
					boolean active = selected == category;
					if (active || hover > 0) {
						int fill = active ? 0xFF153F56 : Draw.alpha(s.buttonHover, hover);
						Draw.round(g, x, y, x + w, y + h, 2, fill);
					}
					if (active) {
						g.fill(x, y + 4, x + 2, y + h - 4, s.accent);
					}
					g.texture(ICONS[index], x + 7, y + 4, 11, 11, 0, 0, 32, 32, 32, 32);
					g.text(category.title(), x + 22, y + 5, active ? s.accent : s.muted, false);
				}

				@Override
				public boolean click(double mx, double my, int button) {
					if (button != Keys.MOUSE_LEFT) return false;
					change.accept(category);
					return true;
				}
			}).bounds(x + 6, top + 32 + i * 20, divider - x - 12, 18);
		}
		if (bottom - top >= 212) {
			String player = ArcticClient.platform().playerName();
			String accounts = Lang.t("mods.accounts");
			host.add(new Widget() {
				@Override
				protected void draw(Gfx g, Style s, int mx, int my, float dt) {
					Skin.button(g, s, x, y, w, h, true, hover);
					g.texture("asset:icons/account", x + 4, y + (h - 16) / 2, 16, 16, 0, 0, 32, 32, 32, 32);
					g.text(Draw.fit(g, player, w - 27), x + 23, y + 6, s.text, false);
					g.text(Draw.fit(g, accounts, w - 27), x + 23, y + 20, s.muted, false);
				}

				@Override
				public boolean click(double mx, double my, int button) {
					if (button != Keys.MOUSE_LEFT) return false;
					openAccount.run();
					return true;
				}
			}).bounds(x + 6, bottom - 44, divider - x - 12, 36);
		}
		return top + 24;
	}

	void draw(Gfx g, Style s) {
		Skin.panel(g, s, x, y, right, bottom);
		if (!sidebar) return;
		g.fill(x + 1, y + 1, divider, bottom - 1, 0xBD070F1A);
		g.fill(divider, top, divider + 1, bottom - 8, s.border);
		g.fill(x + 6, top + 24, divider - 6, top + 25, s.border);
		BrandMark.draw(g, x + 9, top + 3, 14);
		g.text("§lArctic", x + 29, top + 1, s.text, false); // i18n-ignore: brand
		g.text(Lang.t("mods.client"), x + 29, top + 12, s.muted, false);
	}
}
