package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Toggle;

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
		return "Menu Style";
	}

	@Override
	public String hint() {
		return "How Minecraft's menus look. Changes apply right away.";
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
		host.add(new Toggle("Fancy", "Smooth font and rounded, smooth shapes everywhere", Form.binding(() -> c.fancy, on -> {
			c.fancy = on;
			ArcticClient.platform().setSmoothFont(on);
		}))).bounds(x, top + CARD_H + 8, w, ROW);
		textY = top + CARD_H + 8 + ROW + 10;
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		Style chosen = ArcticClient.style();
		g.text(chosen.name, x, textY, s.accent, false);
		g.text(Draw.fit(g, chosen.description, w, x, textY + 12), x, textY + 12, s.muted, false);
	}
}
