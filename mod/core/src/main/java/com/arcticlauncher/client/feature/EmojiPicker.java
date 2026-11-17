package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Lang;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** The chat's :) button, grouped picture picker, and primary shortcode insertion. */
public final class EmojiPicker {
	private static final int BUTTON = 11;
	private static final int COLUMNS = 8;
	private static final int CELL = 18;
	private static final int PAD = 4;
	private static final int HEADING = 14;
	private static boolean open;

	/** Shared geometry for drawing, hit testing, and the preview dump. Coordinates are panel-relative. */
	public static final class Cell {
		public final EmoteAtlas.Emote emote;
		public final int x, y;
		private Cell(EmoteAtlas.Emote emote, int x, int y) { this.emote = emote; this.x = x; this.y = y; }
	}
	private static final List<Cell> CELLS = new ArrayList<Cell>();
	private static final int PANEL_H;
	static {
		String group = "";
		int y = PAD, column = 0;
		for (EmoteAtlas.Emote emote : EmoteAtlas.all()) {
			if (!group.equals(emote.group)) {
				if (column != 0) y += CELL;
				y += HEADING;
				column = 0;
				group = emote.group;
			}
			CELLS.add(new Cell(emote, PAD + column * CELL, y));
			if (++column == COLUMNS) { column = 0; y += CELL; }
		}
		PANEL_H = y + (column == 0 ? 0 : CELL) + PAD;
	}
	private EmojiPicker() {}
	public static List<Cell> layout() { return Collections.unmodifiableList(CELLS); }
	public static int panelWidth() { return COLUMNS * CELL + PAD * 2; }
	public static int panelHeight() { return PANEL_H; }
	private static int buttonX(int w) { return w - BUTTON - 4; }
	private static int buttonY(int h) { return h - BUTTON - 3; }
	private static float panelScale(int w, int h) {
		return Math.min(1f, Math.min((w - 8f) / panelWidth(), (buttonY(h) - 5f) / PANEL_H));
	}
	private static int panelX(int w, float scale) { return w - (int) Math.ceil(panelWidth() * scale) - 4; }
	private static int panelY(int h, float scale) { return buttonY(h) - (int) Math.ceil(PANEL_H * scale) - 4; }
	public static void closed() { open = false; }
	public static void render(Gfx g, ClientConfig c, int mx, int my, int w, int h) {
		if (!c.emojiButton) return;
		int bx = buttonX(w), by = buttonY(h);
		g.fill(bx, by, bx + BUTTON, by + BUTTON, open || inside(mx, my, bx, by, BUTTON, BUTTON) ? 0xB05CE1E6 : 0x80000000);
		g.text(":)", bx + 2, by + 2, 0xFFFFFFFF, false);
		if (!open) return;
		g.newLayer();
		float scale = panelScale(w, h);
		int px = panelX(w, scale), py = panelY(h, scale);
		int localX = (int) ((mx - px) / scale), localY = (int) ((my - py) / scale);
		g.push();
		g.translate(px, py);
		g.scale(scale);
		g.fill(-1, -1, panelWidth() + 1, PANEL_H + 1, 0xFF5CE1E6);
		g.fill(0, 0, panelWidth(), PANEL_H, 0xF0101820);
		String group = "", tip = null;
		for (Cell cell : CELLS) {
			if (!group.equals(cell.emote.group)) {
				group = cell.emote.group;
				g.text(Lang.t(group), PAD, cell.y - HEADING + 3, 0xFFCEDDE5, false);
			}
			int x = cell.x, y = cell.y;
			if (inside(localX, localY, x, y, CELL, CELL)) {
				g.fill(x, y, x + CELL, y + CELL, 0x405CE1E6);
				tip = cell.emote.shortcode();
			}
			cell.emote.draw(g, x + 1, y + 1, CELL - 2);
		}
		g.pop();
		if (tip != null) {
			int tx = Math.max(1, Math.min(mx, w - g.textWidth(tip) - 8));
			int ty = Math.max(1, my - 14);
			g.newLayer();
			g.fill(tx - 2, ty - 2, tx + g.textWidth(tip) + 3, ty + 10, 0xFF101820);
			g.text(tip, tx, ty, 0xFFFFFFFF, false);
		}
	}
	private static boolean inside(int x, int y, int left, int top, int w, int h) {
		return x >= left && x < left + w && y >= top && y < top + h;
	}
	/** Null lets chat handle the click; empty consumes a panel/button click. */
	public static String click(ClientConfig c, int x, int y, int w, int h) {
		if (!c.emojiButton) return null;
		if (inside(x, y, buttonX(w), buttonY(h), BUTTON, BUTTON)) { open = !open; return ""; }
		if (!open) return null;
		float scale = panelScale(w, h);
		int px = panelX(w, scale), py = panelY(h, scale);
		if (x < px || y < py) { open = false; return null; }
		x = (int) ((x - px) / scale);
		y = (int) ((y - py) / scale);
		if (!inside(x, y, 0, 0, panelWidth(), PANEL_H)) { open = false; return null; }
		for (Cell cell : CELLS) {
			if (inside(x, y, cell.x, cell.y, CELL, CELL)) return cell.emote.shortcode();
		}
		return "";
	}
}
