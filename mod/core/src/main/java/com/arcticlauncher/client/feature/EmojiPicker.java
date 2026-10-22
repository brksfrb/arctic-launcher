package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Emoji button in the chat: a small smiley at the right end of the chat line opens a box of symbols;
 * clicking one types it into the line (the same symbols as the Emoji menu).
 */
public final class EmojiPicker {
	private static final int BUTTON = 11;
	private static final int COLUMNS = 12;
	private static final int CELL = 14;
	private static final int PAD = 4;

	private static boolean open;
	private static List<String> symbols;

	private EmojiPicker() {}

	private static List<String> symbols() {
		if (symbols == null) {
			List<String> out = new ArrayList<String>();
			for (Map.Entry<String, String> group : Emoji.groups().entrySet()) {
				for (String s : group.getValue().split(" ")) {
					if (!s.isEmpty()) {
						out.add(s);
					}
				}
			}
			symbols = out;
		}
		return symbols;
	}

	private static int buttonX(int screenW) {
		return screenW - BUTTON - 4;
	}

	private static int buttonY(int screenH) {
		return screenH - BUTTON - 3;
	}

	private static int panelW() {
		return COLUMNS * CELL + PAD * 2;
	}

	private static int panelH() {
		return ((symbols().size() + COLUMNS - 1) / COLUMNS) * CELL + PAD * 2;
	}

	private static int panelX(int screenW) {
		return screenW - panelW() - 4;
	}

	private static int panelY(int screenH) {
		return buttonY(screenH) - panelH() - 4;
	}

	/** The chat screen closed: the box too. */
	public static void closed() {
		open = false;
	}

	/** Draws the button, and the box when it is open. */
	public static void render(Gfx g, ClientConfig c, int mouseX, int mouseY, int screenW, int screenH) {
		if (!c.emojiButton) {
			return;
		}
		int bx = buttonX(screenW);
		int by = buttonY(screenH);
		boolean over = mouseX >= bx && mouseX < bx + BUTTON && mouseY >= by && mouseY < by + BUTTON;
		g.fill(bx, by, bx + BUTTON, by + BUTTON, open || over ? 0xB05CE1E6 : 0x80000000);
		g.text(":)", bx + 2, by + 2, 0xFFFFFFFF, false);
		if (!open) {
			return;
		}
		g.newLayer();
		int px = panelX(screenW);
		int py = panelY(screenH);
		g.fill(px - 1, py - 1, px + panelW() + 1, py + panelH() + 1, 0xFF5CE1E6);
		g.fill(px, py, px + panelW(), py + panelH(), 0xF0101820);
		List<String> all = symbols();
		for (int i = 0; i < all.size(); i++) {
			int cx = px + PAD + i % COLUMNS * CELL;
			int cy = py + PAD + i / COLUMNS * CELL;
			boolean hover = mouseX >= cx && mouseX < cx + CELL && mouseY >= cy && mouseY < cy + CELL;
			if (hover) {
				g.fill(cx, cy, cx + CELL, cy + CELL, 0x405CE1E6);
			}
			g.text(all.get(i), cx + (CELL - g.textWidth(all.get(i))) / 2, cy + 3, 0xFFFFFFFF, false);
		}
	}

	/**
	 * A click on the chat screen. Null: not ours (the chat gets it). Otherwise it was used: the symbol to type
	 * into the chat line, or "" for a click that only opened or closed the box.
	 */
	public static String click(ClientConfig c, int x, int y, int screenW, int screenH) {
		if (!c.emojiButton) {
			return null;
		}
		int bx = buttonX(screenW);
		int by = buttonY(screenH);
		if (x >= bx && x < bx + BUTTON && y >= by && y < by + BUTTON) {
			open = !open;
			return "";
		}
		if (!open) {
			return null;
		}
		int px = panelX(screenW);
		int py = panelY(screenH);
		if (x < px || x >= px + panelW() || y < py || y >= py + panelH()) {
			open = false;
			return null;
		}
		int col = (x - px - PAD) / CELL;
		int row = (y - py - PAD) / CELL;
		int index = row * COLUMNS + col;
		if (x - px < PAD || y - py < PAD || col >= COLUMNS || index < 0 || index >= symbols().size()) {
			return "";
		}
		return symbols().get(index);
	}
}
