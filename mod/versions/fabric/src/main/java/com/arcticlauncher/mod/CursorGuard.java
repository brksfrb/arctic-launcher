package com.arcticlauncher.mod;

import net.minecraft.client.Minecraft;

/**
 * A screen is open but the game still has the mouse grabbed (the pointer hidden and locked): the
 * pointer is given back. Seen after the loading screen in fullscreen, where the title screen showed
 * with no cursor. A screen always wants the pointer, so this can only help.
 */
public final class CursorGuard {
	private static boolean logged;

	private CursorGuard() {}

	public static void tick() {
		Minecraft mc = Minecraft.getInstance();
		if (Compat.screen() == null || !mc.mouseHandler.isMouseGrabbed()) {
			return;
		}
		mc.mouseHandler.releaseMouse();
		if (!logged) {
			logged = true;
			com.arcticlauncher.client.ArcticClient.platform().log(false, "Arctic: gave the mouse pointer back to an open screen");
		}
	}
}
