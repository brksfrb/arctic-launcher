package com.arcticlauncher.client.menu;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.TextField;

/**
 * The Mods screen: every Arctic feature as a card, with a search box and
 * category chips. Cards switch mods on and off and open their settings.
 */
final class ModsScreen extends Page {
	private static final int CARD_H = 74;
	private static final int GAP = 6;
	private static final int MIN_CARD_W = 96;
	private static final int HUD_W = 60;
	private static final int SEARCH_W = 150;
	private static final int FIELD_H = 18;
	private static final int CHIP_H = 16;
	private static final int CHIP_GAP = 4;
	private static final int LOGO = 18;
	private static final int MAX_QUERY = 32;

	/** Kept while the game runs. */
	private static Mod.Category category = Mod.Category.ALL;
	private static String query = "";
	private static int scrollOffset;

	private final TextField search = new TextField("Search mods", MAX_QUERY, TextField.ANY);
	/** Typing in the search box: rebuild the grid but keep typing. */
	private boolean keepSearchFocus;
	private Scroll scroll;
	private int px;
	private int py;
	private int pw;
	private int ph;
	private int gridTop;
	private int shown;

	ModsScreen() {
		search.text(query);
		search.onChange(() -> {
			query = search.text();
			scrollOffset = 0;
			keepSearchFocus = true;
			rebuild();
		});
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void build() {
		pw = Math.min(TabPage.MAX_W, width - TabPage.MARGIN * 2);
		ph = Math.min(TabPage.MAX_H, height - TabPage.MARGIN * 2);
		px = (width - pw) / 2;
		py = (height - ph) / 2;
		int x = px + TabPage.PAD;
		int w = pw - TabPage.PAD * 2;
		if (scroll != null) {
			scrollOffset = scroll.offset();
		}
		add(search).bounds(px + pw - TabPage.PAD - SEARCH_W - 22, py + (TabPage.HEADER - FIELD_H) / 2, SEARCH_W, FIELD_H);
		add(new Button("×", this::close)).bounds(px + pw - TabPage.PAD - FIELD_H, py + (TabPage.HEADER - FIELD_H) / 2, FIELD_H, FIELD_H);
		if (ArcticClient.platform().hasFeatures()) {
			// The HUD layout, from anywhere (outside a world it shows sample values).
			add(new Button("Edit HUD", () -> ArcticClient.platform().openPage(new HudEditor())))
					.bounds(px + pw - TabPage.PAD - SEARCH_W - 22 - CHIP_GAP - HUD_W, py + (TabPage.HEADER - FIELD_H) / 2, HUD_W, FIELD_H);
		}
		if (keepSearchFocus) {
			keepSearchFocus = false;
			focus(search);
		}
		int cy = py + TabPage.HEADER + 6;
		int cx = x;
		for (final Mod.Category cat : Mod.Category.values()) {
			int cw = Math.max(34, 12 + chipWidth(cat.title));
			add(new Button(cat.title, () -> {
				category = cat;
				scrollOffset = 0;
				rebuild();
			}).selected(cat == category)).bounds(cx, cy, cw, CHIP_H);
			cx += cw + CHIP_GAP;
		}
		gridTop = cy + CHIP_H + 8;
		int bottom = py + ph - TabPage.PAD;
		scroll = new Scroll(x, gridTop, x + w, bottom, scrollOffset);
		int cols = Math.max(1, (w - 6 + GAP) / (MIN_CARD_W + GAP));
		int cardW = (w - 6 - (cols - 1) * GAP) / cols;
		List<Mod> mods = visible();
		shown = mods.size();
		for (int i = 0; i < mods.size(); i++) {
			final Mod m = mods.get(i);
			ModCard card = new ModCard(m, () -> ArcticClient.platform().openPage(Menus.pageFor(m, this)), () -> {});
			card.bounds(x + (i % cols) * (cardW + GAP), gridTop + (i / cols) * (CARD_H + GAP), cardW, CARD_H);
			scroll.add(add(card));
		}
		scroll.settle();
	}

	/** The chips' text is short; a rough width keeps them tidy before the first frame. */
	private static int chipWidth(String title) {
		return title.length() * 6;
	}

	private static List<Mod> visible() {
		List<Mod> out = new ArrayList<Mod>();
		for (Mod m : ModCatalog.all()) {
			if ((category == Mod.Category.ALL || m.category == category) && m.matches(query)) {
				out.add(m);
			}
		}
		return out;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + pw, py + ph);
		int x = px + TabPage.PAD;
		g.texture("icon", x, py + (TabPage.HEADER - LOGO) / 2, LOGO, LOGO, 0, 0, 256, 256, 256, 256);
		g.text("§lArctic", x + LOGO + 6, py + 10, s.text, false);
		g.text("Mods", x + LOGO + 6, py + 22, s.muted, false);
		g.fill(x, py + TabPage.HEADER, px + pw - TabPage.PAD, py + TabPage.HEADER + 1, s.border);
		if (shown == 0) {
			Draw.centered(g, "No mods match \"" + query + "\".", px + pw / 2, gridTop + 30, s.muted, false);
		}
	}

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		if (scroll != null) {
			scroll.drawBar(g, s);
		}
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		return scroll != null && scroll.wheel(mx, my, amount);
	}

	/** Typing anywhere starts a search (like Lunar's). */
	@Override
	public boolean charTyped(int codepoint) {
		if (!typing() && codepoint > ' ') {
			focus(search);
		}
		return super.charTyped(codepoint);
	}

	@Override
	protected boolean rightShift() {
		close();
		return true;
	}

	@Override
	public void close() {
		ArcticClient.saveConfig();
		super.close();
	}
}
