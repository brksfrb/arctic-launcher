package com.arcticlauncher.mod;

import com.arcticlauncher.client.Keys;
import com.mojang.blaze3d.platform.InputConstants;

/**
 * 26.3 reads input through SDL, so its mouse buttons (left = 1) and keys
 * (SDL scancodes) differ from the core's GLFW-style codes. Everything
 * coming from the game goes through here.
 */
public final class Input {
	/** Not a code the core knows. */
	public static final int UNKNOWN = -1;

	private Input() {}

	public static int mouse(int button) {
		return switch (button) {
			case InputConstants.MOUSE_BUTTON_LEFT -> Keys.MOUSE_LEFT;
			case InputConstants.MOUSE_BUTTON_RIGHT -> Keys.MOUSE_RIGHT;
			default -> UNKNOWN;
		};
	}

	public static int key(int key) {
		return switch (key) {
			case InputConstants.KEY_ESCAPE -> Keys.ESCAPE;
			case InputConstants.KEY_RETURN -> Keys.ENTER;
			case InputConstants.KEY_RSHIFT -> Keys.RIGHT_SHIFT;
			case InputConstants.KEY_BACKSPACE -> Keys.BACKSPACE;
			default -> UNKNOWN;
		};
	}
}
