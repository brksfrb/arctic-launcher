package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.social.Social;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Widget;

/**
 * What the one "Arctic" button opens (title screen, pause menu): Configure HUD as the big tile, then Mods,
 * Replays and Friends with a line about each, and Presets, Accounts and Menu Style as small icon tiles. Right
 * Shift still goes straight to the HUD screen or the Mods menu.
 */
final class ArcticHub extends Page {
	private static final int PANEL_W = 264;
	private static final int PAD = 12;
	private static final int HEADER_H = 26;
	private static final int HERO_H = 38;
	private static final int TILE_H = 32;
	private static final int SMALL_H = 40;
	private static final int GAP = 6;
	private static final int ICON = 16;

	private int px;
	private int py;
	private int bottom;

	/** Unread chat messages across friends. */
	static int unreadChat() {
		Social social = ArcticClient.social();
		int n = 0;
		if (social != null && social.available()) {
			for (Social.Friend f : social.friends()) {
				n += f.unread;
			}
		}
		return n;
	}

	/** Features of this version nobody has opened yet. */
	static int newFeatures() {
		return ModCatalog.newCount();
	}

	/** The count shown on the Arctic button: unread chat plus new features. */
	public static int badge() {
		return unreadChat() + newFeatures();
	}

	@Override
	protected void build() {
		int inner = PANEL_W - PAD * 2;
		px = (width - PANEL_W) / 2;
		py = Math.max(24, height / 5);
		int x = px + PAD;
		int y = py + HEADER_H;
		add(new Tile("hud", "Configure HUD", "Move, resize and style what is on screen", 0, true,
				() -> ArcticClient.platform().openPage(new HudEditor()))).bounds(x, y, inner, HERO_H);
		y += HERO_H + GAP;
		int news = newFeatures();
		String modsLine = news > 0 ? news + " new to try" : "Every feature and its settings";
		int half = (inner - GAP) / 2;
		boolean replays = com.arcticlauncher.client.replay.Replays.canWatch();
		add(new Tile("mods", "Mods", modsLine, news, false,
				() -> ArcticClient.platform().openPage(new ModsScreen()))).bounds(x, y, replays ? half : inner, TILE_H);
		if (replays) {
			add(new Tile("replay", "Replays", "Watch and export clips", 0, false,
					() -> ArcticClient.platform().openPage(Menus.page("replays")))).bounds(x + half + GAP, y, half, TILE_H);
		}
		y += TILE_H + GAP;
		int chat = unreadChat();
		add(new Tile("friends", "Friends", chat > 0 ? chat + " unread" : "Chat and play together", chat, false,
				() -> ArcticClient.platform().openPage(Menus.page("friends")))).bounds(x, y, inner, TILE_H);
		y += TILE_H + GAP;
		int third = (inner - GAP * 2) / 3;
		add(new Tile("palette", "Presets", null, 0, false,
				() -> ArcticClient.platform().openPage(new Presets()))).bounds(x, y, third, SMALL_H);
		add(new Tile("account", "Accounts", null, 0, false,
				() -> ArcticClient.platform().openPage(Menus.page("account")))).bounds(x + third + GAP, y, third, SMALL_H);
		add(new Tile("style", "Menu Style", null, 0, false,
				() -> ArcticClient.platform().openPage(Menus.page("style")))).bounds(x + (third + GAP) * 2, y, inner - (third + GAP) * 2, SMALL_H);
		y += SMALL_H + PAD;
		bottom = y;
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + PANEL_W, bottom);
		g.texture("icon", px + PAD, py + 6, 14, 14, 0, 0, 256, 256, 256, 256);
		g.text("Arctic", px + PAD + 19, py + 10, s.text, false);
		String version = ArcticClient.platform().minecraftVersion();
		if (version != null) {
			g.text(version, px + PANEL_W - PAD - g.textWidth(version), py + 10, s.muted, false);
		}
	}

	/** One destination: an icon, its name, a line about it, and a count when something waits there. */
	private static final class Tile extends Widget {
		private final String icon;
		private final String title;
		private final String line;
		private final int count;
		private final boolean hero;
		private final Runnable action;

		Tile(String icon, String title, String line, int count, boolean hero, Runnable action) {
			this.icon = icon;
			this.title = title;
			this.line = line;
			this.count = count;
			this.hero = hero;
			this.action = action;
		}

		@Override
		protected void draw(Gfx g, Style s, int mx, int my, float dt) {
			if (hero) {
				Skin.primary(g, s, x, y, w, h, enabled, hover);
			} else {
				Skin.button(g, s, x, y, w, h, enabled, hover);
			}
			int main = hero ? s.onAccent : s.text;
			int quiet = hero ? Draw.alpha(s.onAccent, 0.8f) : s.muted;
			String path = "asset:icons/" + icon;
			if (line == null) {
				// Small tile: the icon above its name.
				g.texture(path, x + (w - ICON) / 2, y + 6, ICON, ICON, 0, 0, 64, 64, 64, 64);
				Draw.centered(g, Draw.fit(g, title, w - 6), x + w / 2, y + h - 13, main, false);
				return;
			}
			int pad = 8;
			g.texture(path, x + pad, y + (h - ICON) / 2, ICON, ICON, 0, 0, 64, 64, 64, 64);
			int textX = x + pad + ICON + 8;
			int room = x + w - textX - pad - (count > 0 ? 16 : 0);
			g.text(Draw.fit(g, title, room), textX, y + h / 2 - 9, main, false);
			g.text(Draw.fit(g, line, room), textX, y + h / 2 + 1, quiet, false);
			if (count > 0) {
				String n = count > 99 ? "99+" : String.valueOf(count);
				int bw = Math.max(12, g.textWidth(n) + 6);
				int bx = x + w - pad - bw;
				int by = y + (h - 12) / 2;
				Draw.round(g, bx, by, bx + bw, by + 12, 6, hero ? s.onAccent : s.accent);
				Draw.centered(g, n, bx + bw / 2, by + 2, hero ? s.accent : s.onAccent, false);
			}
		}

		@Override
		public boolean click(double mx, double my, int button) {
			if (!enabled || button != Keys.MOUSE_LEFT) {
				return false;
			}
			action.run();
			return true;
		}
	}
}
