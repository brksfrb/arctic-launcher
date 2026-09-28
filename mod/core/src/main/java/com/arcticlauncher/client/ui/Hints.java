package com.arcticlauncher.client.ui;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * The whole text of a line that was shortened with "…", shown in a tooltip
 * once the mouse rests on it for a moment. Text drawn through
 * {@link Draw#fit(Gfx, String, int, int, int)} offers itself here; the page
 * draws the tooltip last, above everything.
 */
public final class Hints {
	/** How long the mouse rests before the tooltip shows. */
	private static final long DELAY_MS = 400;
	private static final int MAX_WIDTH = 220;
	private static final int LINE = 10;
	private static final int PAD = 4;

	private static int mouseX;
	private static int mouseY;
	/** Offered this frame under the mouse. */
	private static String offered;
	/** Shown (or waiting to show) since {@link #since}. */
	private static String current;
	private static long since;

	private Hints() {}

	/** A new frame: forget last frame's offer. */
	public static void frame(int mx, int my) {
		mouseX = mx;
		mouseY = my;
		offered = null;
	}

	/** {@code full} was shortened to fit the box at (x, y, w, h). */
	public static void offer(String full, int x, int y, int w, int h) {
		if (mouseX >= x && mouseX < x + w && mouseY >= y && mouseY < y + h) {
			offered = full;
		}
	}

	/** Draw the tooltip if the mouse has rested on a shortened line. */
	public static void draw(Gfx g, Style s, int screenW, int screenH) {
		if (offered == null) {
			current = null;
			return;
		}
		long now = System.currentTimeMillis();
		if (!offered.equals(current)) {
			current = offered;
			since = now;
		}
		if (now - since < DELAY_MS) {
			return;
		}
		List<String> lines = wrap(g, current, MAX_WIDTH);
		int w = 0;
		for (String l : lines) {
			w = Math.max(w, g.textWidth(l));
		}
		w += PAD * 2;
		int h = lines.size() * LINE + PAD * 2 - 1;
		int x = Math.max(2, Math.min(mouseX + 8, screenW - w - 2));
		int y = mouseY - h - 4 < 2 ? mouseY + 12 : mouseY - h - 4;
		y = Math.max(2, Math.min(y, screenH - h - 2));
		Draw.round(g, x, y, x + w, y + h, 3, 0xF0000000 | (s.panel & 0xFFFFFF));
		Draw.outline(g, x, y, x + w, y + h, 3, s.border);
		for (int i = 0; i < lines.size(); i++) {
			g.text(lines.get(i), x + PAD, y + PAD + i * LINE, s.text, false);
		}
	}

	/** Words wrapped to lines no wider than {@code width}. */
	public static List<String> wrap(Gfx g, String text, int width) {
		List<String> lines = new ArrayList<String>();
		StringBuilder line = new StringBuilder();
		for (String word : text.split(" ")) {
			String next = line.length() == 0 ? word : line + " " + word;
			if (line.length() > 0 && g.textWidth(next) > width) {
				lines.add(line.toString());
				line = new StringBuilder(word);
			} else {
				line = new StringBuilder(next);
			}
		}
		if (line.length() > 0) {
			lines.add(line.toString());
		}
		return lines;
	}
}
