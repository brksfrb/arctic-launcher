package com.arcticlauncher.mod.mixin;

import com.mojang.blaze3d.platform.Window;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * "Start the game maximized" (the launcher sets arctic.maximized): the game maximizes its own
 * window, the one way that works on every system (Wayland lets no other program do it).
 */
@Mixin(Window.class)
abstract class WindowMaximizeMixin {
	@Inject(method = "<init>", at = @At("TAIL"))
	private void arctic$maximize(CallbackInfo ci) {
		Window window = (Window) (Object) this;
		if (!Boolean.getBoolean("arctic.maximized")) {
			return;
		}
		//#if MC >= 26.3
		// 26.3 opens its window through SDL.
		long handle = window.handle();
		if ((org.lwjgl.sdl.SDLVideo.SDL_GetWindowFlags(handle) & org.lwjgl.sdl.SDLVideo.SDL_WINDOW_FULLSCREEN) == 0) {
			org.lwjgl.sdl.SDLVideo.SDL_MaximizeWindow(handle);
		}
		//#else
		if (window.isFullscreen()) {
			return;
		}
		// The new window's context is current once it's made.
		long handle = org.lwjgl.glfw.GLFW.glfwGetCurrentContext();
		if (handle != 0) {
			org.lwjgl.glfw.GLFW.glfwMaximizeWindow(handle);
		}
		//#endif
	}
}
