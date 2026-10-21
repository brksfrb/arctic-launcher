package com.arcticlauncher.mod;

import net.minecraft.client.Minecraft;

/**
 * A screen is open but the pointer is hidden: it is given back. Two ways it gets lost: the game still
 * has the mouse grabbed (seen after the loading screen in fullscreen), or the window itself has the
 * cursor hidden or captured while the game thinks it is free (seen when coming back to a fullscreen
 * game with Alt+Tab; GLFW versions only). A screen always wants the pointer, so this can only help.
 */
public final class CursorGuard {
	private static int logged;

	private CursorGuard() {}

	public static void tick() {
		if (Compat.screen() == null) {
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		boolean grabbed = mc.mouseHandler.isMouseGrabbed();
		int mode = windowCursorMode();
		if (!grabbed && mode == 0) {
			return;
		}
		if (grabbed) {
			mc.mouseHandler.releaseMouse();
		}
		restoreWindowCursor();
		// The first few times only, with what was wrong: it tells which of the two it was.
		if (logged < 5) {
			logged++;
			com.arcticlauncher.client.ArcticClient.platform().log(false,
				"Arctic: gave the mouse pointer back to an open screen (game grabbed: " + grabbed + ", window cursor hidden: " + (mode != 0) + ")");
		}
	}

	/** 1 when the window has the cursor hidden or captured, 0 when it is normal (always 0 where SDL replaced GLFW). */
	private static int windowCursorMode() {
		//#if MC >= 26.3
		return 0;
		//#else
		long window = Compat.windowHandle();
		return org.lwjgl.glfw.GLFW.glfwGetInputMode(window, org.lwjgl.glfw.GLFW.GLFW_CURSOR) == org.lwjgl.glfw.GLFW.GLFW_CURSOR_NORMAL ? 0 : 1;
		//#endif
	}

	private static void restoreWindowCursor() {
		//#if MC < 26.3
		org.lwjgl.glfw.GLFW.glfwSetInputMode(Compat.windowHandle(), org.lwjgl.glfw.GLFW.GLFW_CURSOR, org.lwjgl.glfw.GLFW.GLFW_CURSOR_NORMAL);
		//#endif
	}
}
