package com.arcticlauncher.mod.mixin;

import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * "Start the game maximized" (the launcher sets arctic.maximized): the game maximizes its own
 * window, the one way that works on every system (Wayland lets no other program do it).
 */
@Mixin(targets = "com.mojang.blaze3d.platform.Window")
abstract class WindowMaximizeMixin {
	@Inject(method = "<init>", at = @At("TAIL"))
	private void arctic$maximize(CallbackInfo ci) {
		if (!Boolean.getBoolean("arctic.maximized")) {
			return;
		}
		// The new window's context is current once it's made.
		long window = GLFW.glfwGetCurrentContext();
		if (window != 0 && GLFW.glfwGetWindowMonitor(window) == 0) {
			GLFW.glfwMaximizeWindow(window);
		}
	}
}
