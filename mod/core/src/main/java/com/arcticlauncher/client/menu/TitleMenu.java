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
	/**
	 * The wordmark is a picture made for each GUI scale (1–8) and drawn one
	 * image pixel per screen pixel: scaled-up text blurs into gray edges.
	 * Sizes of {@code assets/arctic/wordmark-<scale>.png}.
	 */
	private static final int[][] WORDMARK = {
		{88, 18}, {175, 37}, {263, 55}, {351, 74}, {438, 92}, {526, 111}, {613, 129}, {701, 147},
	};

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
				ArcticClient.platform().openPage(Menus.page("mods"));
			}
		})).bounds(left + third + GAP, y, third, BUTTON_H);
		add(new Button("Quit", run(MenuAction.QUIT))).bounds(left + (third + GAP) * 2, y, BUTTON_W - (third + GAP) * 2, BUTTON_H);
		corner("Accessibility", MenuAction.ACCESSIBILITY, width - 6);
		corner("Language", MenuAction.LANGUAGE, width - 6 - 76 - GAP);
		int right = width - 6 - (76 + GAP) * 2;
		if (com.arcticlauncher.client.replay.Replays.canWatch()) {
			add(new Button("Replays", () -> ArcticClient.platform().openPage(Menus.page("replays")))).bounds(right - 76, 6, 76, 16);
			right -= 76 + GAP;
		}
		if (ArcticClient.platform().hasFeatures()) {
			add(new Button("Edit HUD", () -> ArcticClient.platform().openPage(new HudEditor()))).bounds(right - 76, 6, 76, 16);
		}
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
		float ps = g.pixelScale();
		int scale = Math.max(1, Math.min(WORDMARK.length, Math.round(ps)));
		int w = WORDMARK[scale - 1][0];
		int h = WORDMARK[scale - 1][1];
		int wordTop = top + ICON + 6;
		g.push();
		g.scale(1f / ps);
		g.texture("asset:wordmark-" + scale, Math.round(width * ps / 2f - w / 2f), Math.round(wordTop * ps), w, h, 0, 0,
				w, h, w, h);
		g.pop();
		String sub = "Minecraft " + ArcticClient.platform().minecraftVersion();
		Draw.centered(g, sub, width / 2, wordTop + Math.round(h / ps) + 5, s.accent, true);
	}

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		int y = height - 12;
		g.text("Arctic Client", 6, y, s.muted, true);
		String who = ArcticClient.platform().playerName();
		g.text(who, width - 6 - g.textWidth(who), y, s.muted, true);
	}
}
