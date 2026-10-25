package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Shulker Peek: hover a shulker box (or a chest, barrel, hopper... carrying its items) in your inventory
 * or a chest and see what is inside as a small grid beside the cursor. Only what the item itself carries
 * is shown (the game already has it). Holding Shift shows the compact preview (what is inside, packed);
 * Shift and Alt together show every slot where it lies. The grid takes the shulker box's colour.
 */
public final class ContainerPeek {
	/** Setting: shown while hovering, every slot. */
	public static final int ALWAYS = 0;
	/** Setting: shown while Shift is held (compact; with Alt, every slot). */
	public static final int HOLD_SHIFT = 1;

	private static final int COLS = 9;
	private static final int CELL = 18;
	private static final int PAD = 4;
	private static final int GAP_TO_MOUSE = 14;
	private static final String SHULKER_SUFFIX = "_shulker_box";

	private ContainerPeek() {}

	/** How many slots the container item has, by its item id path ("chest", "hopper"...); 27 when unknown. */
	public static int slotsFor(String path) {
		if (path == null) {
			return 27;
		}
		if (path.equals("dispenser") || path.equals("dropper") || path.equals("crafter")) {
			return 9;
		}
		if (path.equals("hopper") || path.equals("brewing_stand")) {
			return 5;
		}
		if (path.equals("furnace") || path.equals("smoker") || path.equals("blast_furnace")) {
			return 3;
		}
		return 27;
	}

	/** The colour (0xRRGGBB) a shulker box item gives the preview, 0 for anything else. */
	public static int tintFor(String path) {
		if (path == null || !path.endsWith("shulker_box")) {
			return 0;
		}
		String dye = path.endsWith(SHULKER_SUFFIX) ? path.substring(0, path.length() - SHULKER_SUFFIX.length()) : "";
		switch (dye) {
			case "white": return 0xF9FFFE;
			case "orange": return 0xF9801D;
			case "magenta": return 0xC74EBD;
			case "light_blue": return 0x3AB3DA;
			case "yellow": return 0xFED83D;
			case "lime": return 0x80C71F;
			case "pink": return 0xF38BAA;
			case "gray": return 0x474F52;
			case "light_gray": return 0x9D9D97;
			case "cyan": return 0x169C9C;
			case "purple": return 0x8932B8;
			case "blue": return 0x3C44AA;
			case "brown": return 0x835432;
			case "green": return 0x5E7C16;
			case "red": return 0xB02E26;
			case "black": return 0x1D1D21;
			default: return 0x976997;
		}
	}

	private static boolean down(Platform p, String left, String right) {
		return p.isKeyDown(left) || p.isKeyDown(right);
	}

	/** The item under the cursor: draws what it holds when it is a container and the setting says so. */
	public static void hover(Gfx g, int mouseX, int mouseY, Object stack) {
		ClientConfig c = ArcticClient.config();
		Platform p = ArcticClient.platform();
		if (c == null || p == null || !c.shulkerPeek || stack == null) {
			return;
		}
		boolean full = c.shulkerPeekMode == ALWAYS;
		if (!full) {
			if (!down(p, "key.keyboard.left.shift", "key.keyboard.right.shift")) {
				return;
			}
			full = down(p, "key.keyboard.left.alt", "key.keyboard.right.alt");
		}
		draw(g, mouseX, mouseY, p.peek(stack), p.peekTint(stack), full);
	}

	/**
	 * Draws the container's slots ({@code items}: the game's stacks, null for empty) beside the cursor:
	 * every slot where it lies when {@code full}, else just what is inside, packed. {@code tint} is
	 * 0xRRGGBB, or 0 for the plain look.
	 */
	public static void draw(Gfx g, int mouseX, int mouseY, Object[] items, int tint, boolean full) {
		if (items == null || items.length == 0) {
			return;
		}
		Object[] shown = full ? items : packed(items);
		int cols = full && items.length < COLS ? (items.length == 3 ? 3 : items.length) : COLS;
		if (full && items.length == 9) {
			cols = 3;
		}
		cols = Math.max(1, Math.min(cols, shown.length));
		int rows = (shown.length + cols - 1) / cols;
		int w = cols * CELL + PAD * 2;
		int h = rows * CELL + PAD * 2;
		int x = mouseX - w - GAP_TO_MOUSE;
		if (x < 2) {
			x = Math.min(g.width() - w - 2, mouseX + 190);
		}
		int y = Math.max(2, Math.min(g.height() - h - 2, mouseY - h / 2));
		g.newLayer();
		int frame = tint == 0 ? 0xFF1A1A2E : 0xFF000000 | tint;
		int back = tint == 0 ? 0xF0100010 : 0xF0000000 | darker(tint);
		g.fill(x - 1, y - 1, x + w + 1, y + h + 1, frame);
		g.fill(x, y, x + w, y + h, back);
		for (int i = 0; i < shown.length; i++) {
			int cx = x + PAD + i % cols * CELL;
			int cy = y + PAD + i / cols * CELL;
			g.fill(cx, cy, cx + CELL - 2, cy + CELL - 2, 0x40FFFFFF);
			if (shown[i] != null) {
				g.item(shown[i], cx, cy);
			}
		}
	}

	/** The stacks without the gaps between them. */
	private static Object[] packed(Object[] items) {
		int n = 0;
		for (Object item : items) {
			if (item != null) {
				n++;
			}
		}
		Object[] out = new Object[n];
		int at = 0;
		for (Object item : items) {
			if (item != null) {
				out[at++] = item;
			}
		}
		return out;
	}

	/** A dark version of the colour for the window's background (a quarter of it). */
	private static int darker(int rgb) {
		int r = (rgb >> 16 & 255) / 4;
		int gr = (rgb >> 8 & 255) / 4;
		int b = (rgb & 255) / 4;
		return r << 16 | gr << 8 | b;
	}
}
