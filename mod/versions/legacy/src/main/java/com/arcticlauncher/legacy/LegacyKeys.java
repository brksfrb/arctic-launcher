package com.arcticlauncher.legacy;

import java.util.HashMap;
import java.util.Map;

import com.arcticlauncher.client.Keys;
import net.minecraft.client.option.GameOptions;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;

/**
 * Keys on old Minecraft (LWJGL 2): Arctic's settings name keys the modern
 * way ("key.keyboard.c", "key.mouse.left") so they carry across versions;
 * here those names become LWJGL 2 key codes (mouse buttons are -100 + n,
 * as Minecraft 1.8 stores them) and back.
 */
public final class LegacyKeys {
	private static final String KEYBOARD = "key.keyboard.";
	private static final String MOUSE = "key.mouse.";
	/** Minecraft's code for mouse button n. */
	private static final int MOUSE_OFFSET = -100;

	/** Modern key name (after "key.keyboard.") → LWJGL 2 key name, where they differ. */
	private static final Map<String, String> SPECIAL = new HashMap<String, String>();
	private static final Map<String, String> SPECIAL_BACK = new HashMap<String, String>();

	static {
		String[][] pairs = {
				{"left.shift", "LSHIFT"}, {"right.shift", "RSHIFT"}, {"left.control", "LCONTROL"}, {"right.control", "RCONTROL"},
				{"left.alt", "LMENU"}, {"right.alt", "RMENU"}, {"space", "SPACE"}, {"tab", "TAB"}, {"enter", "RETURN"},
				{"escape", "ESCAPE"}, {"backspace", "BACK"}, {"grave.accent", "GRAVE"}, {"caps.lock", "CAPITAL"},
				{"left.bracket", "LBRACKET"}, {"right.bracket", "RBRACKET"}, {"semicolon", "SEMICOLON"}, {"apostrophe", "APOSTROPHE"},
				{"comma", "COMMA"}, {"period", "PERIOD"}, {"slash", "SLASH"}, {"backslash", "BACKSLASH"}, {"minus", "MINUS"},
				{"equal", "EQUALS"}, {"up", "UP"}, {"down", "DOWN"}, {"left", "LEFT"}, {"right", "RIGHT"}, {"insert", "INSERT"},
				{"delete", "DELETE"}, {"home", "HOME"}, {"end", "END"}, {"page.up", "PRIOR"}, {"page.down", "NEXT"},
				{"left.win", "LMETA"}, {"right.win", "RMETA"}, {"menu", "APPS"}, {"keypad.enter", "NUMPADENTER"},
				{"keypad.add", "ADD"}, {"keypad.subtract", "SUBTRACT"}, {"keypad.multiply", "MULTIPLY"}, {"keypad.divide", "DIVIDE"},
				{"keypad.decimal", "DECIMAL"},
		};
		for (String[] p : pairs) {
			SPECIAL.put(p[0], p[1]);
			SPECIAL_BACK.put(p[1], p[0]);
		}
		for (int i = 0; i <= 9; i++) {
			SPECIAL.put("keypad." + i, "NUMPAD" + i);
			SPECIAL_BACK.put("NUMPAD" + i, "keypad." + i);
		}
	}

	private LegacyKeys() {}

	/** LWJGL 2 code for a modern key name; 0 for none/unknown. */
	public static int code(String name) {
		if (name == null) {
			return 0;
		}
		if (name.startsWith(MOUSE)) {
			String button = name.substring(MOUSE.length());
			if ("left".equals(button)) {
				return MOUSE_OFFSET;
			}
			if ("right".equals(button)) {
				return MOUSE_OFFSET + 1;
			}
			if ("middle".equals(button)) {
				return MOUSE_OFFSET + 2;
			}
			try {
				return MOUSE_OFFSET + Integer.parseInt(button) - 1;
			} catch (NumberFormatException e) {
				return 0;
			}
		}
		if (!name.startsWith(KEYBOARD)) {
			return 0;
		}
		String key = name.substring(KEYBOARD.length());
		if ("unknown".equals(key)) {
			return 0;
		}
		String lwjgl = SPECIAL.containsKey(key) ? SPECIAL.get(key) : key.toUpperCase(java.util.Locale.ROOT);
		return Keyboard.getKeyIndex(lwjgl);
	}

	/** Modern key name for an LWJGL 2 code (mouse: -100 + button). */
	public static String name(int code) {
		if (code < 0) {
			int button = code - MOUSE_OFFSET;
			return MOUSE + (button == 0 ? "left" : button == 1 ? "right" : button == 2 ? "middle" : String.valueOf(button + 1));
		}
		String lwjgl = code == 0 ? null : Keyboard.getKeyName(code);
		if (lwjgl == null || "NONE".equals(lwjgl)) {
			return KEYBOARD + "unknown";
		}
		String modern = SPECIAL_BACK.get(lwjgl);
		return KEYBOARD + (modern != null ? modern : lwjgl.toLowerCase(java.util.Locale.ROOT));
	}

	/** Held right now (keyboard or mouse). */
	public static boolean isDown(String name) {
		return isDown(code(name));
	}

	public static boolean isDown(int code) {
		if (code == 0) {
			return false;
		}
		if (code < 0) {
			int button = code - MOUSE_OFFSET;
			return button < Mouse.getButtonCount() && Mouse.isButtonDown(button);
		}
		return code < Keyboard.KEYBOARD_SIZE && Keyboard.isKeyDown(code);
	}

	/** What the game calls the key ("C", "Button 4"). */
	public static String label(String name) {
		int code = code(name);
		return code == 0 ? "None" : GameOptions.getFormattedNameForKeyCode(code);
	}

	/** The core's key code for an LWJGL 2 key (the few it acts on). */
	public static int core(int lwjgl) {
		switch (lwjgl) {
			case Keyboard.KEY_ESCAPE:
				return Keys.ESCAPE;
			case Keyboard.KEY_RETURN:
			case Keyboard.KEY_NUMPADENTER:
				return Keys.ENTER;
			case Keyboard.KEY_RSHIFT:
				return Keys.RIGHT_SHIFT;
			case Keyboard.KEY_BACK:
				return Keys.BACKSPACE;
			case Keyboard.KEY_TAB:
				return Keys.TAB;
			case Keyboard.KEY_V:
				return Keys.V;
			case Keyboard.KEY_A:
				return Keys.A;
			// The replay's controls.
			case Keyboard.KEY_P:
				return 80;
			case Keyboard.KEY_K:
				return 75;
			case Keyboard.KEY_RIGHT:
				return 262;
			case Keyboard.KEY_LEFT:
				return 263;
			case Keyboard.KEY_DOWN:
				return 264;
			case Keyboard.KEY_UP:
				return 265;
			default:
				return -1;
		}
	}

	/** The core's mouse button for an LWJGL 2 button. */
	public static int coreMouse(int button) {
		// LWJGL 2 numbers buttons like the core does: 0 left, 1 right, 2 middle, 3+ side buttons.
		return button >= 0 ? button : -1;
	}
}
