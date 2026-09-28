package com.arcticlauncher.mod;

import com.arcticlauncher.client.Keys;

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
		switch (button) {
			case com.arcticlauncher.mod.compat.KeyCodes.MOUSE_LEFT:
				return Keys.MOUSE_LEFT;
			case com.arcticlauncher.mod.compat.KeyCodes.MOUSE_RIGHT:
				return Keys.MOUSE_RIGHT;
			default:
				return UNKNOWN;
		}
	}

	public static int key(int key) {
		switch (key) {
			case com.arcticlauncher.mod.compat.KeyCodes.ESCAPE:
				return Keys.ESCAPE;
			case com.arcticlauncher.mod.compat.KeyCodes.ENTER:
				return Keys.ENTER;
			case com.arcticlauncher.mod.compat.KeyCodes.RIGHT_SHIFT:
				return Keys.RIGHT_SHIFT;
			case com.arcticlauncher.mod.compat.KeyCodes.BACKSPACE:
				return Keys.BACKSPACE;
			case com.arcticlauncher.mod.compat.KeyCodes.TAB:
				return Keys.TAB;
			case com.arcticlauncher.mod.compat.KeyCodes.V:
				return Keys.V;
			case com.arcticlauncher.mod.compat.KeyCodes.A:
				return Keys.A;
			// The replay's controls.
			case com.arcticlauncher.mod.compat.KeyCodes.P:
				return Keys.P;
			case com.arcticlauncher.mod.compat.KeyCodes.K:
				return Keys.K;
			case com.arcticlauncher.mod.compat.KeyCodes.LEFT:
				return Keys.LEFT;
			case com.arcticlauncher.mod.compat.KeyCodes.RIGHT:
				return Keys.RIGHT;
			case com.arcticlauncher.mod.compat.KeyCodes.UP:
				return Keys.UP;
			case com.arcticlauncher.mod.compat.KeyCodes.DOWN:
				return Keys.DOWN;
			default:
				return UNKNOWN;
		}
	}
}
