package com.arcticlauncher.mod;

import com.arcticlauncher.client.controller.Gamepad;

/**
 * The first connected game controller, in the standard layout. GLFW's gamepad API up to
 * 26.2 (it knows hundreds of controllers' layouts); SDL's from 26.3, whose window is SDL.
 */
final class Gamepads {
	/** Looking for a controller that isn't there is slow on some systems: only this often. */
	private static final long SCAN_MS = 2000;

	private static long lastScan;

	private Gamepads() {}

	//#if MC >= 26.3
	/** SDL button numbers, in {@link Gamepad}'s order. */
	private static final int[] BUTTONS = {
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_SOUTH,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_EAST,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_WEST,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_NORTH,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_LEFT_SHOULDER,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_BACK,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_START,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_GUIDE,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_LEFT_STICK,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_RIGHT_STICK,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_DPAD_UP,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_DPAD_RIGHT,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_DPAD_DOWN,
			org.lwjgl.sdl.SDLGamepad.SDL_GAMEPAD_BUTTON_DPAD_LEFT,
	};
	private static boolean started;
	private static long open;

	static boolean read(Gamepad pad) {
		if (!started) {
			started = true;
			// The game starts SDL for its window only.
			org.lwjgl.sdl.SDLInit.SDL_InitSubSystem(org.lwjgl.sdl.SDLInit.SDL_INIT_GAMEPAD);
		}
		if (open != 0 && !org.lwjgl.sdl.SDLGamepad.SDL_GamepadConnected(open)) {
			org.lwjgl.sdl.SDLGamepad.SDL_CloseGamepad(open);
			open = 0;
		}
		if (open == 0 && !openFirst()) {
			return false;
		}
		for (int i = 0; i < Gamepad.AXES; i++) {
			pad.axes[i] = org.lwjgl.sdl.SDLGamepad.SDL_GetGamepadAxis(open, i) / 32767f;
		}
		for (int i = 0; i < Gamepad.BUTTONS; i++) {
			pad.buttons[i] = org.lwjgl.sdl.SDLGamepad.SDL_GetGamepadButton(open, BUTTONS[i]);
		}
		return true;
	}

	private static boolean openFirst() {
		long now = System.currentTimeMillis();
		if (now - lastScan < SCAN_MS) {
			return false;
		}
		lastScan = now;
		if (!org.lwjgl.sdl.SDLGamepad.SDL_HasGamepad()) {
			return false;
		}
		java.nio.IntBuffer ids = org.lwjgl.sdl.SDLGamepad.SDL_GetGamepads();
		if (ids == null) {
			return false;
		}
		try {
			if (ids.remaining() > 0) {
				open = org.lwjgl.sdl.SDLGamepad.SDL_OpenGamepad(ids.get(0));
			}
		} finally {
			org.lwjgl.sdl.SDLStdinc.SDL_free(ids);
		}
		return open != 0;
	}
	//#else
	/** The controller in use (GLFW joystick id), or -1. */
	private static int jid = -1;
	private static org.lwjgl.glfw.GLFWGamepadState state;

	static boolean read(Gamepad pad) {
		if (jid >= 0 && !org.lwjgl.glfw.GLFW.glfwJoystickIsGamepad(jid)) {
			jid = -1;
		}
		if (jid < 0 && !findFirst()) {
			return false;
		}
		if (state == null) {
			// Allocated once (older LWJGL has no stack allocator for it).
			state = org.lwjgl.glfw.GLFWGamepadState.create();
		}
		if (!org.lwjgl.glfw.GLFW.glfwGetGamepadState(jid, state)) {
			return false;
		}
		for (int i = 0; i < Gamepad.AXES; i++) {
			float v = state.axes(i);
			// GLFW's triggers go from -1 (let go) to 1.
			pad.axes[i] = i >= Gamepad.LT ? (v + 1) / 2 : v;
		}
		// GLFW's button order is the standard one.
		for (int i = 0; i < Gamepad.BUTTONS; i++) {
			pad.buttons[i] = state.buttons(i) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
		}
		return true;
	}

	private static boolean findFirst() {
		long now = System.currentTimeMillis();
		if (now - lastScan < SCAN_MS) {
			return false;
		}
		lastScan = now;
		for (int id = org.lwjgl.glfw.GLFW.GLFW_JOYSTICK_1; id <= org.lwjgl.glfw.GLFW.GLFW_JOYSTICK_LAST; id++) {
			if (org.lwjgl.glfw.GLFW.glfwJoystickIsGamepad(id)) {
				jid = id;
				return true;
			}
		}
		return false;
	}
	//#endif
}
