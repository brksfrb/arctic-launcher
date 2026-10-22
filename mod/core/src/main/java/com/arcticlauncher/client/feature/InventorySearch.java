package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import java.util.Locale;

/**
 * Search in chests and the inventory: Ctrl+F opens a search line over any container screen, the items
 * whose names contain what you type stay bright and the rest are dimmed. Only the screen's own
 * contents are looked at (what the server already sent), nothing more.
 */
public final class InventorySearch {
	private static final int MAX_LENGTH = 40;
	/** The state is dropped when no container screen has drawn for this long (it was closed). */
	private static final long CLOSED_MS = 400;
	private static final int ACTION_RELEASE = 0;
	private static final int ACTION_REPEAT = 2;

	private static boolean active;
	private static String text = "";
	private static String lower = "";
	private static long seenAt;

	private InventorySearch() {}

	/** A container screen is being drawn (starts over when the last one closed a while ago). */
	public static void seen() {
		long now = System.currentTimeMillis();
		if (now - seenAt > CLOSED_MS) {
			reset();
		}
		seenAt = now;
	}

	static void reset() {
		active = false;
		text = "";
		lower = "";
	}

	/** Whether a search is narrowing the items down right now. */
	public static boolean filtering() {
		return !lower.isEmpty();
	}

	/** A key in a container screen; true when the search used it (so the game does not). */
	public static boolean key(ClientConfig c, int key, int action, boolean ctrl) {
		if (!c.containerSearch) {
			return false;
		}
		if (!active) {
			if (key == Keys.F && ctrl) {
				if (action == 1) {
					active = true;
				}
				return true;
			}
			return false;
		}
		if (action == ACTION_RELEASE) {
			return true;
		}
		if (key == Keys.ESCAPE) {
			reset();
		} else if (key == Keys.ENTER) {
			active = false;
		} else if (key == Keys.BACKSPACE && !text.isEmpty()) {
			set(text.substring(0, text.length() - 1));
		} else if (key == Keys.F && ctrl && action != ACTION_REPEAT) {
			active = false;
		}
		return true;
	}

	/** A typed character in a container screen; true when the search took it. */
	public static boolean typed(ClientConfig c, int codePoint) {
		if (!c.containerSearch || !active) {
			return false;
		}
		if (codePoint >= 32 && codePoint != 127 && text.length() < MAX_LENGTH) {
			set(text + new String(Character.toChars(codePoint)));
		}
		return true;
	}

	private static void set(String value) {
		text = value;
		lower = value.trim().toLowerCase(Locale.ROOT);
	}

	/** Does an item with this name stay bright? (Without a search: always.) */
	public static boolean matches(String name) {
		return lower.isEmpty() || name != null && name.toLowerCase(Locale.ROOT).contains(lower);
	}

	/** A slot's look while searching: matches get a frame, items that don't are dimmed. Empty slots are left alone. */
	public static void slot(Gfx g, int x, int y, String itemName) {
		if (itemName == null) {
			return;
		}
		if (matches(itemName)) {
			g.fill(x - 1, y - 1, x + 17, y, 0xFF5CE1E6);
			g.fill(x - 1, y + 16, x + 17, y + 17, 0xFF5CE1E6);
			g.fill(x - 1, y, x, y + 16, 0xFF5CE1E6);
			g.fill(x + 16, y, x + 17, y + 16, 0xFF5CE1E6);
		} else {
			g.fill(x, y, x + 16, y + 16, 0xA0000000);
		}
	}

	/** The search line at the top of the screen (while searching or with a search set). */
	public static void render(Gfx g, ClientConfig c) {
		if (!c.containerSearch || !active && lower.isEmpty()) {
			return;
		}
		String shown = "Search: " + text + (active && System.currentTimeMillis() / 500 % 2 == 0 ? "_" : "");
		int w = Math.max(120, g.textWidth(shown) + 14);
		int x = (g.width() - w) / 2;
		g.fill(x - 1, 5, x + w + 1, 21, active ? 0xFF5CE1E6 : 0xFF555555);
		g.fill(x, 6, x + w, 20, 0xE0101820);
		g.text(shown, x + 7, 9, 0xFFFFFFFF, true);
		if (!active) {
			g.text("Ctrl+F to edit", x + w + 6, 9, 0xFFAAAAAA, true);
		}
	}
}
