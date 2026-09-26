package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;

/** Arctic's title screen, in place of Minecraft's. */
public final class TitleMenu extends Page {
	private static final int BUTTON_W = 200;
	private static final int BUTTON_H = 20;
	private static final int GAP = 4;
	private static final int ICON = 24;
	private static final float WORDMARK_SCALE = 2.5f;
	private static final String WORDMARK = "§lARCTIC";

	@Override
	public boolean ownBackground() {
		return true;
	}

	@Override
	public boolean closeOnEscape() {
		return false;
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	private int buttonsTop() {
		return height / 4 + 48;
	}

	private int logoTop() {
		return Math.max(6, height / 4 - 36);
	}

	@Override
	protected void build() {
		int left = width / 2 - BUTTON_W / 2;
		int y = buttonsTop();
		add(new Button("Singleplayer", run(MenuAction.SINGLEPLAYER)).primary()).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP;
		add(new Button("Multiplayer", run(MenuAction.MULTIPLAYER))).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP;
		add(new Button("Realms", run(MenuAction.REALMS))).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP * 3;
		int third = (BUTTON_W - GAP * 2) / 3;
		add(new Button("Options", run(MenuAction.OPTIONS))).bounds(left, y, third, BUTTON_H);
		add(new Button("Arctic", new Runnable() {
			@Override
			public void run() {
				ArcticClient.platform().openPage(new ArcticMenu());
			}
		})).bounds(left + third + GAP, y, third, BUTTON_H);
		add(new Button("Quit", run(MenuAction.QUIT))).bounds(left + (third + GAP) * 2, y, BUTTON_W - (third + GAP) * 2, BUTTON_H);
		corner("Accessibility", MenuAction.ACCESSIBILITY, width - 6);
		corner("Language", MenuAction.LANGUAGE, width - 6 - 76 - GAP);
	}

	/** A small button in the top-right corner, right edge at {@code right}. */
	private void corner(String label, MenuAction action, int right) {
		int w = 76;
		add(new Button(label, run(action))).bounds(right - w, 6, w, 16);
	}

	private static Runnable run(final MenuAction action) {
		return new Runnable() {
			@Override
			public void run() {
				ArcticClient.platform().action(action);
			}
		};
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		int top = logoTop();
		g.texture("icon", width / 2 - ICON / 2, top, ICON, ICON, 0, 0, 256, 256, 256, 256);
		int wordW = Math.round(g.textWidth(WORDMARK) * WORDMARK_SCALE);
		Draw.big(g, WORDMARK, width / 2 - wordW / 2, top + ICON + 4, WORDMARK_SCALE, s.text);
		String sub = "Minecraft " + ArcticClient.platform().minecraftVersion();
		Draw.centered(g, sub, width / 2, top + ICON + 8 + Math.round(8 * WORDMARK_SCALE), s.accent, true);
	}

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		int y = height - 12;
		g.text("Arctic Client", 6, y, s.muted, true);
		String who = ArcticClient.platform().playerName();
		g.text(who, width - 6 - g.textWidth(who), y, s.muted, true);
	}
}
