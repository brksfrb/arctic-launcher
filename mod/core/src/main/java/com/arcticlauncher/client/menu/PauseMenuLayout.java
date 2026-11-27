package com.arcticlauncher.client.menu;

import java.util.BitSet;

/** Finds room for a full-width action in the pause menu's column. */
public final class PauseMenuLayout {
	private static final int GAP = 4;
	private final int left;
	private final int right;
	private final int height;
	private final int lastY;
	private final BitSet blocked = new BitSet();

	public PauseMenuLayout(int x, int width, int height, int screenHeight) {
		this.left = x;
		this.right = x + width;
		this.height = height;
		this.lastY = screenHeight - height - GAP;
	}

	public void avoid(int x, int y, int width, int height) {
		if (x >= right + GAP || x + width <= left - GAP) return;
		// Block every top edge that would put this action within the widget's padding.
		int from = Math.max(GAP, y - this.height - GAP + 1);
		int to = Math.min(lastY + 1, y + height + GAP);
		if (from < to) blocked.set(from, to);
	}

	public int findY(int menuTop, int menuBottom) {
		int below = blocked.nextClearBit(Math.max(GAP, menuBottom + GAP));
		if (below <= lastY) return below;
		// A long menu may only have room between existing rows or above the first action.
		int within = blocked.nextClearBit(Math.max(GAP, menuTop));
		if (within <= lastY) return within;
		int before = blocked.previousClearBit(Math.max(-1, Math.min(lastY, menuTop - 1)));
		return before >= GAP ? before : -1;
	}
}
