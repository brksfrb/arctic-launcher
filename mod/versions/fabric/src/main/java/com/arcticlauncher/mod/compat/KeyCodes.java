package com.arcticlauncher.mod.compat;

//#if MC >= 1.17
import com.mojang.blaze3d.platform.InputConstants;
//#endif

/**
 * The game's own key and button codes (GLFW's until 26.3 moved to SDL), as
 * constants on every version: before 1.17 InputConstants had none, so
 * these are GLFW's numbers there.
 */
public final class KeyCodes {
	//#if MC >= 1.17
	public static final int ESCAPE = InputConstants.KEY_ESCAPE;
	public static final int ENTER = InputConstants.KEY_RETURN;
	public static final int TAB = InputConstants.KEY_TAB;
	public static final int BACKSPACE = InputConstants.KEY_BACKSPACE;
	public static final int RIGHT_SHIFT = InputConstants.KEY_RSHIFT;
	public static final int K = InputConstants.KEY_K;
	public static final int P = InputConstants.KEY_P;
	public static final int V = InputConstants.KEY_V;
	public static final int A = InputConstants.KEY_A;
	public static final int LEFT = InputConstants.KEY_LEFT;
	public static final int RIGHT = InputConstants.KEY_RIGHT;
	public static final int UP = InputConstants.KEY_UP;
	public static final int DOWN = InputConstants.KEY_DOWN;
	public static final int PRESS = InputConstants.PRESS;
	public static final int RELEASE = InputConstants.RELEASE;
	public static final int MOUSE_LEFT = InputConstants.MOUSE_BUTTON_LEFT;
	public static final int MOUSE_RIGHT = InputConstants.MOUSE_BUTTON_RIGHT;
	//#else
	public static final int ESCAPE = 256;
	public static final int ENTER = 257;
	public static final int TAB = 258;
	public static final int BACKSPACE = 259;
	public static final int RIGHT_SHIFT = 344;
	public static final int K = 75;
	public static final int P = 80;
	public static final int V = 86;
	public static final int A = 65;
	public static final int LEFT = 263;
	public static final int RIGHT = 262;
	public static final int UP = 265;
	public static final int DOWN = 264;
	public static final int PRESS = 1;
	public static final int RELEASE = 0;
	public static final int MOUSE_LEFT = 0;
	public static final int MOUSE_RIGHT = 1;
	//#endif

	private KeyCodes() {}
}
