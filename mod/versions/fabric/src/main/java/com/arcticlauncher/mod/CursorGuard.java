package com.arcticlauncher.mod;

import net.minecraft.client.Minecraft;
import org.lwjgl.glfw.GLFW;

/**
 * A screen is open but the pointer is hidden: it is given back. Two ways it gets lost: the game still
 * has the mouse grabbed (seen after the loading screen in fullscreen), or the window itself has the
 * cursor hidden or captured while the game thinks it is free (seen when coming back to a fullscreen
 * game with Alt+Tab). A screen always wants the pointer, so this can only help.
 */
public final class CursorGuard {
	private static int logged;

	private CursorGuard() {}

	public static void tick() {
		if (Compat.screen() == null) {
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		long window = Compat.windowHandle();
		int mode = GLFW.glfwGetInputMode(window, GLFW.GLFW_CURSOR);
		boolean grabbed = mc.mouseHandler.isMouseGrabbed();
		if (!grabbed && mode == GLFW.GLFW_CURSOR_NORMAL) {
			return;
		}
		if (grabbed) {
			mc.mouseHandler.releaseMouse();
		}
		if (GLFW.glfwGetInputMode(window, GLFW.GLFW_CURSOR) != GLFW.GLFW_CURSOR_NORMAL) {
			GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_NORMAL);
		}
		// The first few times only, with what was wrong: it tells which of the two it was.
		if (logged < 5) {
			logged++;
			com.arcticlauncher.client.ArcticClient.platform().log(false,
				"Arctic: gave the mouse pointer back to an open screen (game grabbed: " + grabbed + ", window cursor mode: 0x" + Integer.toHexString(mode) + ")");
		}
	}
}
