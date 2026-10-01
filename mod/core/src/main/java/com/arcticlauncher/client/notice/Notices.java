package com.arcticlauncher.client.notice;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * Small pop-ups in the top-right corner: a friend's message, an invite, a
 * screenshot just taken (with its picture). Drawn over the game and over
 * Arctic's menus; each slides in, stays a few seconds and fades.
 */
public final class Notices {
	private static final long SHOW_MS = 5000;
	/** Notices with a button stay longer, so there's time to free the mouse and click. */
	private static final long SHOW_ACTION_MS = 10000;
	private static final int BUTTON_H = 14;
	private static final int BUTTON_W = 60;
	private static final long FADE_MS = 400;
	private static final int MAX_SHOWN = 3;
	private static final int WIDTH = 170;
	private static final int PAD = 5;
	private static final int MARGIN = 4;
	private static final int LINE = 10;
	private static final int GAP = 4;

	/** A notice; {@code image} is a texture key ({@code look:<hash>}) or null. */
	public static final class Notice {
		final String title;
		final String body;
		final String image;
		final int imageW;
		final int imageH;
		/** A button on the notice (clickable whenever the mouse is free), or null. */
		String action;
		Runnable onAction;
		/** Where the button was last drawn: {x0, y0, x1, y1}. */
		int[] button;
		/** Where the notice was last drawn: a click on it (off the button) closes it. */
		int[] card;
		long shownAt;

		Notice(String title, String body, String image, int imageW, int imageH) {
			this.title = title;
			this.body = body;
			this.image = image;
			this.imageW = imageW;
			this.imageH = imageH;
		}

		long showMs() {
			return action != null ? SHOW_ACTION_MS : SHOW_MS;
		}
	}

	private static final List<Notice> QUEUE = new ArrayList<Notice>();

	private Notices() {}

	public static void post(String title, String body) {
		post(title, body, null, 0, 0);
	}

	/** With a picture drawn {@code w}×{@code h} GUI pixels (fits the width). */
	public static void post(String title, String body, String image, int w, int h) {
		post(title, body, image, w, h, null, null);
	}

	/** With a picture and a button ({@code action} labels it; {@code onAction} runs on click). */
	public static synchronized void post(String title, String body, String image, int w, int h, String action, Runnable onAction) {
		Notice n = new Notice(title, body == null ? "" : body, image, w, h);
		n.action = action;
		n.onAction = onAction;
		QUEUE.add(n);
		while (QUEUE.size() > 8) {
			QUEUE.remove(0);
		}
	}

	/**
	 * A click at GUI coordinates (while a screen frees the mouse): a notice's
	 * button runs it; anywhere else on a notice closes it. True if it hit one.
	 */
	public static synchronized boolean click(double mx, double my) {
		for (int i = 0; i < QUEUE.size(); i++) {
			Notice n = QUEUE.get(i);
			int[] b = n.button;
			if (b != null && n.onAction != null && inside(b, mx, my)) {
				Runnable run = n.onAction;
				n.onAction = null;
				run.run();
				return true;
			}
			if (n.card != null && inside(n.card, mx, my)) {
				QUEUE.remove(i);
				return true;
			}
		}
		return false;
	}

	private static boolean inside(int[] r, double mx, double my) {
		return mx >= r[0] && mx < r[2] && my >= r[1] && my < r[3];
	}

	/** Change a notice's button text after its click ("Copied"). */
	public static synchronized void relabel(String oldLabel, String newLabel) {
		for (Notice n : QUEUE) {
			if (oldLabel.equals(n.action)) {
				n.action = newLabel;
			}
		}
	}

	public static synchronized void render(Gfx g, Style s) {
		long now = System.currentTimeMillis();
		com.arcticlauncher.client.Platform platform = com.arcticlauncher.client.ArcticClient.platform();
		int y = MARGIN + (platform == null ? 0 : platform.toastsBottom());
		int shown = 0;
		for (int i = 0; i < QUEUE.size() && shown < MAX_SHOWN; i++) {
			Notice n = QUEUE.get(i);
			if (n.shownAt == 0) {
				n.shownAt = now;
			}
			long age = now - n.shownAt;
			long show = n.showMs();
			if (age > show + FADE_MS) {
				QUEUE.remove(i--);
				continue;
			}
			float alpha = age > show ? 1f - (age - show) / (float) FADE_MS : 1f;
			float slide = Math.min(1f, age / 180f);
			y += draw(g, s, n, y, alpha, slide) + GAP;
			shown++;
		}
	}

	private static int draw(Gfx g, Style s, Notice n, int y, float alpha, float slide) {
		List<String> lines = com.arcticlauncher.client.ui.Hints.wrap(g, n.body, WIDTH - PAD * 2);
		if (lines.size() > 3) {
			lines = lines.subList(0, 3);
		}
		int imgW = 0;
		int imgH = 0;
		if (n.image != null && n.imageW > 0) {
			imgW = WIDTH - PAD * 2;
			imgH = Math.round(n.imageH * imgW / (float) n.imageW);
		}
		int buttonRow = n.action != null ? BUTTON_H + 3 : 0;
		int h = PAD + LINE + (n.body.isEmpty() ? 0 : lines.size() * LINE) + (imgH > 0 ? imgH + 3 : 0) + buttonRow + PAD - 1;
		int x = g.width() - MARGIN - Math.round(WIDTH * (0.3f + 0.7f * slide));
		Draw.round(g, x, y, x + WIDTH, y + h, 4, Draw.alpha(0xF0000000 | (s.panel & 0xFFFFFF), alpha));
		Draw.outline(g, x, y, x + WIDTH, y + h, 4, Draw.alpha(s.accent, alpha));
		n.card = new int[] {x, y, x + WIDTH, y + h};
		// A close mark in the corner (the whole card closes on click).
		Draw.centered(g, "×", x + WIDTH - 7, y + 3, Draw.alpha(s.muted, alpha), false);
		int ty = y + PAD;
		if (imgH > 0) {
			g.texture(n.image, x + PAD, ty, imgW, imgH, 0, 0, n.imageW, n.imageH, n.imageW, n.imageH);
			ty += imgH + 3;
		}
		g.text(Draw.fit(g, n.title, WIDTH - PAD * 2 - 8), x + PAD, ty, Draw.alpha(s.text, alpha), false);
		ty += LINE;
		if (!n.body.isEmpty()) {
			for (String l : lines) {
				g.text(l, x + PAD, ty, Draw.alpha(s.muted, alpha), false);
				ty += LINE;
			}
		}
		if (n.action != null) {
			int bx = x + WIDTH - PAD - BUTTON_W;
			int by = ty + 2;
			boolean live = n.onAction != null;
			Draw.round(g, bx, by, bx + BUTTON_W, by + BUTTON_H, 3, Draw.alpha(live ? s.accent : s.button, alpha));
			Draw.centered(g, n.action, bx + BUTTON_W / 2, by + (BUTTON_H - 8) / 2, Draw.alpha(live ? s.onAccent : s.muted, alpha), false);
			n.button = new int[] {bx, by, bx + BUTTON_W, by + BUTTON_H};
		}
		return h;
	}
}
