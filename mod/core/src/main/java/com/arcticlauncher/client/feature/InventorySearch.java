package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
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
	/** Everything in the line is selected (Ctrl+A): the next letter replaces it, Backspace clears it. */
	private static boolean selectedAll;
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
		selectedAll = false;
	}

	/** What is typed in the search line. */
	public static String query() {
		return text;
	}

	/** Whether the search line is taking keys. */
	public static boolean active() {
		return active;
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
		} else if ((key == Keys.BACKSPACE || key == Keys.DELETE) && selectedAll) {
			set("");
		} else if (key == Keys.BACKSPACE && ctrl && !text.isEmpty()) {
			set(withoutLastWord(text));
		} else if (key == Keys.BACKSPACE && !text.isEmpty()) {
			set(text.substring(0, text.length() - 1));
		} else if (key == Keys.A && ctrl && action != ACTION_REPEAT) {
			selectedAll = !text.isEmpty();
		} else if (key == Keys.C && ctrl && action != ACTION_REPEAT && !text.isEmpty()) {
			ArcticClient.platform().setClipboard(text);
		} else if (key == Keys.X && ctrl && action != ACTION_REPEAT && !text.isEmpty()) {
			ArcticClient.platform().setClipboard(text);
			set("");
		} else if (key == Keys.V && ctrl) {
			String pasted = ArcticClient.platform().clipboard();
			if (pasted != null && !pasted.isEmpty()) {
				StringBuilder cleaned = new StringBuilder();
				for (int i = 0; i < pasted.length(); i++) {
					char ch = pasted.charAt(i);
					cleaned.append(ch < ' ' || ch == 127 ? ' ' : ch);
				}
				String clean = cleaned.toString().trim();
				String base = selectedAll ? "" : text;
				set((base + clean).substring(0, Math.min(MAX_LENGTH, (base + clean).length())));
			}
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
		if (codePoint >= 32 && codePoint != 127 && (selectedAll || text.length() < MAX_LENGTH)) {
			set((selectedAll ? "" : text) + new String(Character.toChars(codePoint)));
		}
		return true;
	}

	/** The text without its last word and the spaces before it (Ctrl+Backspace). */
	static String withoutLastWord(String value) {
		int end = value.length();
		while (end > 0 && value.charAt(end - 1) == ' ') {
			end--;
		}
		while (end > 0 && value.charAt(end - 1) != ' ') {
			end--;
		}
		return value.substring(0, end);
	}

	private static void set(String value) {
		selectedAll = false;
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
		if (selectedAll && active) {
			// Everything selected: a highlight behind the typed text.
			int from = x + 7 + g.textWidth("Search: ");
			g.fill(from - 1, 8, from + g.textWidth(text) + 1, 18, 0xA05CE1E6);
		}
		g.text(shown, x + 7, 9, 0xFFFFFFFF, true);
		if (!active) {
			g.text("Ctrl+F to edit", x + w + 6, 9, 0xFFAAAAAA, true);
		}
	}
}
