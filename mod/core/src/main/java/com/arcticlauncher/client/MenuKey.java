package com.arcticlauncher.client;

import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/**
 * The key that opens and closes the Arctic menu. Right Shift unless the
 * player rebinds it, in Arctic's menu or in Minecraft's Controls screen
 * (both edit the same game key binding, so they never disagree). A binding
 * the menu can't use (none, a mouse button, or a key the menus themselves
 * need) leaves the menu on Right Shift, so it can always be opened.
 */
public final class MenuKey {
	/** Minecraft's name for Right Shift: what the menu key is when nothing else is bound. */
	public static final String DEFAULT = "key.keyboard.right.shift";

	private static final String KEYBOARD = "key.keyboard.";

	/** Keys the pages use themselves (editing, moving, the replay's controls), so they can't open the menu. */
	private static final Set<String> RESERVED = new HashSet<String>(Arrays.asList(
			"escape", "enter", "keypad.enter", "tab", "backspace", "delete", "home", "end", "left", "right", "up", "down",
			"a", "c", "v", "x", "f", "k", "p"));

	/** Used when the game can't hold a key binding (and by tests). */
	private static String remembered = DEFAULT;

	private MenuKey() {}

	/** Can this key (a Minecraft key name) open the menu? */
	public static boolean allowed(String name) {
		if (name == null || !name.startsWith(KEYBOARD) || name.endsWith(".unknown")) {
			return false;
		}
		return !RESERVED.contains(name.substring(KEYBOARD.length()));
	}

	/** The key the menu opens with now (a Minecraft key name). */
	public static String current() {
		Platform p = ArcticClient.platform();
		String bound = p == null ? null : p.menuKeyBinding();
		if (bound == null) {
			bound = remembered;
		}
		return allowed(bound) ? bound : DEFAULT;
	}

	/** Is this key (a Minecraft key name) the menu key right now? */
	public static boolean matches(String name) {
		return name != null && name.equals(current());
	}

	/** The menu key's name as menus show it ("Right Shift", "M"). */
	public static String label() {
		Platform p = ArcticClient.platform();
		return p == null ? DEFAULT : p.keyLabel(current());
	}

	/** Bind the menu key; one the menu can't use puts Right Shift back. */
	public static void set(String name) {
		remembered = allowed(name) ? name : DEFAULT;
		Platform p = ArcticClient.platform();
		if (p != null) {
			p.setMenuKeyBinding(remembered);
		}
	}
}
