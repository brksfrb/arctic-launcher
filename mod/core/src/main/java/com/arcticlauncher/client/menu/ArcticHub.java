package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.social.Social;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;

/**
 * What the one "Arctic" button opens (title screen, pause menu): Configure HUD as the big button, then Mods,
 * Replays and Friends, and Presets, Accounts and Menu Style as small ones. Right Shift still goes straight to
 * the HUD screen or the Mods menu.
 */
final class ArcticHub extends Page {
	private static final int PANEL_W = 240;
	private static final int PAD = 12;
	private static final int BIG_H = 32;
	private static final int ROW_H = 20;
	private static final int GAP = 4;

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
		int y = py + 30;
		final boolean features = ArcticClient.platform().hasFeatures();
		add(new Button("Configure HUD", () -> ArcticClient.platform().openPage(new HudEditor())).primary()).bounds(px + PAD, y, inner, BIG_H);
		y += BIG_H + GAP * 2;
		int half = (inner - GAP) / 2;
		int news = newFeatures();
		add(new Button(news > 0 ? "Mods (" + news + " new)" : "Mods", () -> ArcticClient.platform().openPage(new ModsScreen()))).bounds(px + PAD, y, half, ROW_H + 4);
		if (com.arcticlauncher.client.replay.Replays.canWatch()) {
			add(new Button("Replays", () -> ArcticClient.platform().openPage(Menus.page("replays")))).bounds(px + PAD + half + GAP, y, half, ROW_H + 4);
		}
		y += ROW_H + 4 + GAP;
		int chat = unreadChat();
		add(new Button(chat > 0 ? "Friends (" + chat + ")" : "Friends", () -> ArcticClient.platform().openPage(Menus.page("friends")))
				.selected(chat > 0)).bounds(px + PAD, y, inner, ROW_H + 4);
		y += ROW_H + 4 + GAP * 2;
		int third = (inner - GAP * 2) / 3;
		add(new Button("Presets", () -> ArcticClient.platform().openPage(new Presets()))).bounds(px + PAD, y, third, ROW_H);
		add(new Button("Accounts", () -> ArcticClient.platform().openPage(Menus.page("account")))).bounds(px + PAD + third + GAP, y, third, ROW_H);
		add(new Button("Menu Style", () -> ArcticClient.platform().openPage(Menus.page("style")))).bounds(px + PAD + (third + GAP) * 2, y, inner - (third + GAP) * 2, ROW_H);
		y += ROW_H + PAD;
		bottom = y;
		if (!features) {
			// Nothing to place on the screen without the HUD features (e.g. 1.13).
			bottom = y;
		}
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + PANEL_W, bottom);
		g.text("Arctic", px + PAD, py + 12, s.text, false);
	}
}
