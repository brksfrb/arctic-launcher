package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Toggle;

import com.arcticlauncher.client.Lang;
/** How Minecraft's menus look: a style, and Fancy (smooth font and shapes). */
final class StyleTab implements MenuTab {
	private static final int CARD_H = 66;
	private static final int GAP = 6;
	private static final int ROW = 24;

	private int x;
	private int w;
	private int textY;

	@Override
	public String title() {
		return Lang.t("mods.menu_style");
	}

	@Override
	public String hint() {
		return Lang.t("mods.how_minecrafts_menus_look_changes_apply");
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.w = w;
		Style[] all = Style.ALL;
		int cardW = (w - GAP * (all.length - 1)) / all.length;
		for (int i = 0; i < all.length; i++) {
			final Style style = all[i];
			boolean chosen = style.id.equals(ArcticClient.style().id);
			host.add(new StyleCard(style, chosen, () -> {
				ArcticClient.config().style = style.id;
				ArcticClient.saveConfig();
				host.rebuild();
			})).bounds(x + i * (cardW + GAP), top, cardW, CARD_H);
		}
		final ClientConfig c = ArcticClient.config();
		host.add(new Toggle(Lang.t("mods.fancy"), Lang.t("mods.smooth_font_and_rounded_smooth_shapes"), Form.binding(() -> c.fancy, on -> {
			c.fancy = on;
			ArcticClient.platform().setSmoothFont(on);
		}))).bounds(x, top + CARD_H + 8, w, ROW);
		host.add(new Toggle(Lang.t("mods.smooth_edges"), Lang.t("mods.anti_aliased_letter_edges_off_every_pixel_of"),
				Form.binding(() -> c.fontAntialias, on -> {
					c.fontAntialias = on;
					ArcticClient.platform().reloadFonts();
				}))).bounds(x, top + CARD_H + 8 + ROW + 2, w, ROW);
		textY = top + CARD_H + 8 + (ROW + 2) * 2 + 8;
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		Style chosen = ArcticClient.style();
		g.text(chosen.name, x, textY, s.accent, false);
		g.text(Draw.fit(g, Lang.t(chosen.description), w, x, textY + 12), x, textY + 12, s.muted, false);
	}
}
