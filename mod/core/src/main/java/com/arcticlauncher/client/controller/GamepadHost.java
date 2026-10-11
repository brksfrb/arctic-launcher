package com.arcticlauncher.client.controller;

import com.arcticlauncher.client.GameKey;

/** What playing with a controller needs from the game (each version adapter does it its way). */
public interface GamepadHost {
	/** The first connected controller, into {@code pad}; false when there's none. */
	default boolean readGamepad(Gamepad pad) {
		return false;
	}

	/** Turn the player by these degrees, like moving the mouse does. */
	default void turnPlayer(double yaw, double pitch) {}

	boolean physicalKeyDown(GameKey key);

	void setKeyDown(GameKey key, boolean down);

	/** One press of a control (an attack, opening the inventory...). */
	default void tapKey(GameKey key) {}

	/** Next (+1) or previous (-1) hotbar slot. */
	default void scrollHotbar(int step) {}

	/** What Esc does: close a screen, or pause the game. */
	default void back() {}

	/** The pointer over screens, in window pixels {x, y, window width, window height}; null without one. */
	default double[] cursor() {
		return null;
	}

	default void setCursor(double x, double y) {}

	/** Press (or let go of) a mouse button where the pointer is: 0 left, 1 right. */
	default void mouseButton(int button, boolean down) {}

	/** Scroll the screen under the pointer (positive is up). */
	default void scrollScreen(double amount) {}
}
