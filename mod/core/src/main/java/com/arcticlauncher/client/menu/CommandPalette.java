package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.servers.Servers;
import com.arcticlauncher.client.social.Social;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.TextField;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Ctrl+K: one search box for everything: Arctic's mods and their settings, your saved servers, your
 * friends and a few actions. Type, pick with the arrow keys or the mouse, Enter opens it.
 */
public final class CommandPalette extends Page {
	private static final int PANEL_W = 340;
	private static final int FIELD_H = 20;
	private static final int ROW_H = 26;
	private static final int MAX_ROWS = 8;
	private static final int PAD = 8;
	private static final int MAX_QUERY = 40;

	/** One thing the palette can open. */
	private static final class Entry {
		final String title;
		final String kind;
		final String detail;
		final String haystack;
		final Runnable run;

		Entry(String title, String kind, String detail, String keywords, Runnable run) {
			this.title = title;
			this.kind = kind;
			this.detail = detail;
			this.haystack = (title + " " + kind + " " + detail + " " + keywords).toLowerCase(Locale.ROOT);
			this.run = run;
		}
	}

	private final TextField field = new TextField("Search mods, servers, friends...", MAX_QUERY, TextField.ANY);
	private final List<Entry> all = new ArrayList<Entry>();
	private List<Entry> shown = new ArrayList<Entry>();
	private int selected;
	private int px;
	private int py;

	/** Opens the palette (from a key). */
	public static void open() {
		ArcticClient.platform().openPage(new CommandPalette());
	}

	public CommandPalette() {
		collect();
		field.onChange(() -> {
			selected = 0;
			refilter();
		});
		refilter();
	}

	private void collect() {
		for (final Mod m : ModCatalog.all()) {
			all.add(new Entry(m.name, "Mod", m.description, m.keywords, () -> ArcticClient.platform().openPage(Menus.pageFor(m, null))));
		}
		all.add(new Entry("Edit HUD", "Action", "Move and resize your HUD widgets", "layout widgets",
				() -> ArcticClient.platform().openPage(new HudEditor())));
		all.add(new Entry("Mods", "Action", "All Arctic mods", "menu settings", Menus::mods));
		Social social = ArcticClient.social();
		if (social != null && social.available()) {
			for (Social.Friend f : social.friends()) {
				final String id = f.id;
				all.add(new Entry(f.name, "Friend", f.online ? (f.server != null ? "Online on " + f.server : "Online") : "Offline", "",
						() -> {
							ArcticClient.social().openChat(id);
							Menus.open("friends");
						}));
			}
		}
		try {
			final Servers servers = new Servers(ArcticClient.platform(), ArcticClient.platform().configDir().getAbsoluteFile().getParentFile());
			final List<Servers.Entry> saved = servers.load();
			for (int i = 0; i < saved.size(); i++) {
				final int index = i;
				all.add(new Entry(saved.get(i).name, "Server", "Join " + saved.get(i).address, "multiplayer play " + saved.get(i).address,
						() -> servers.join(index)));
			}
		} catch (RuntimeException e) {
			// No server list to offer.
		}
	}

	private void refilter() {
		String q = field.text().trim().toLowerCase(Locale.ROOT);
		String[] terms = q.isEmpty() ? new String[0] : q.split("\\s+");
		List<Entry> hits = new ArrayList<Entry>();
		List<Entry> later = new ArrayList<Entry>();
		for (Entry e : all) {
			boolean every = true;
			for (String t : terms) {
				if (!e.haystack.contains(t)) {
					every = false;
					break;
				}
			}
			if (!every) {
				continue;
			}
			if (terms.length > 0 && e.title.toLowerCase(Locale.ROOT).startsWith(terms[0])) {
				hits.add(e);
			} else {
				later.add(e);
			}
		}
		hits.addAll(later);
		shown = hits.size() > MAX_ROWS ? new ArrayList<Entry>(hits.subList(0, MAX_ROWS)) : hits;
		selected = Math.min(selected, Math.max(0, shown.size() - 1));
	}

	@Override
	protected void build() {
		px = (width - PANEL_W) / 2;
		py = Math.max(24, height / 5);
		add(field).bounds(px + PAD, py + PAD, PANEL_W - PAD * 2, FIELD_H);
		focus(field);
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected boolean animatesIn() {
		return false;
	}

	@Override
	public boolean keyPressed(int key, int nativeKey) {
		if (key == Keys.ESCAPE) {
			close();
			return true;
		}
		if (key == Keys.DOWN && !shown.isEmpty()) {
			selected = (selected + 1) % shown.size();
			return true;
		}
		if (key == Keys.UP && !shown.isEmpty()) {
			selected = (selected + shown.size() - 1) % shown.size();
			return true;
		}
		if (key == Keys.ENTER) {
			choose(selected);
			return true;
		}
		return super.keyPressed(key, nativeKey);
	}

	private void choose(int index) {
		if (index < 0 || index >= shown.size()) {
			return;
		}
		Entry e = shown.get(index);
		close();
		e.run.run();
	}

	private int rowAt(double mx, double my) {
		int top = py + PAD + FIELD_H + 6;
		if (mx < px || mx >= px + PANEL_W || my < top) {
			return -1;
		}
		int row = (int) ((my - top) / ROW_H);
		return row < shown.size() ? row : -1;
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		int row = rowAt(mx, my);
		if (row >= 0) {
			choose(row);
			return true;
		}
		if (mx < px || mx >= px + PANEL_W || my < py || my >= py + PAD * 2 + FIELD_H + shown.size() * ROW_H + 6) {
			close();
			return true;
		}
		return super.mouseClicked(mx, my, button);
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		int h = PAD * 2 + FIELD_H + (shown.isEmpty() ? 28 : 6 + shown.size() * ROW_H);
		Skin.panel(g, s, px, py, px + PANEL_W, py + h);
	}

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		int top = py + PAD + FIELD_H + 6;
		if (shown.isEmpty()) {
			Draw.centered(g, "Nothing matches.", px + PANEL_W / 2, top + 4, s.muted, false);
			return;
		}
		int hover = rowAt(mx, my);
		for (int i = 0; i < shown.size(); i++) {
			Entry e = shown.get(i);
			int y = top + i * ROW_H;
			if (i == selected || i == hover) {
				g.fill(px + 4, y, px + PANEL_W - 4, y + ROW_H - 2, i == selected ? Draw.alpha(s.accent, 0.35f) : s.buttonHover);
			}
			g.text(Draw.fit(g, e.title, PANEL_W - 90), px + PAD + 2, y + 4, s.text, false);
			g.text(Draw.fit(g, e.detail, PANEL_W - 90), px + PAD + 2, y + 14, s.muted, false);
			g.text(e.kind, px + PANEL_W - PAD - g.textWidth(e.kind), y + 9, s.muted, false);
		}
	}
}
