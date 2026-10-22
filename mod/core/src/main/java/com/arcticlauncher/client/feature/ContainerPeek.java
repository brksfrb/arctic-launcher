package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.gfx.Gfx;

/**
 * Shulker Peek: hover a shulker box in your inventory or a chest and see what is inside it as a small
 * grid beside the cursor. Only what the item itself carries is shown (the game already has it).
 */
public final class ContainerPeek {
	private static final int COLS = 9;
	private static final int CELL = 18;
	private static final int PAD = 4;
	private static final int GAP_TO_MOUSE = 14;

	private ContainerPeek() {}

	/** Draws the box's 27 slots ({@code items}: the game's stacks, null for empty) beside the cursor. */
	public static void draw(Gfx g, int mouseX, int mouseY, Object[] items) {
		if (items == null || items.length == 0) {
			return;
		}
		int rows = (items.length + COLS - 1) / COLS;
		int w = COLS * CELL + PAD * 2;
		int h = rows * CELL + PAD * 2;
		int x = mouseX - w - GAP_TO_MOUSE;
		if (x < 2) {
			x = Math.min(g.width() - w - 2, mouseX + 190);
		}
		int y = Math.max(2, Math.min(g.height() - h - 2, mouseY - h / 2));
		g.newLayer();
		g.fill(x - 1, y - 1, x + w + 1, y + h + 1, 0xFF1A1A2E);
		g.fill(x, y, x + w, y + h, 0xF0100010);
		for (int i = 0; i < items.length; i++) {
			int cx = x + PAD + i % COLS * CELL;
			int cy = y + PAD + i / COLS * CELL;
			g.fill(cx, cy, cx + CELL - 2, cy + CELL - 2, 0x40FFFFFF);
			if (items[i] != null) {
				g.item(items[i], cx, cy);
			}
		}
	}
}
