package com.arcticlauncher.mod.mixin;

import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * "Start the game maximized" (the launcher sets arctic.maximized) before 26.3: once the game is
 * made. Maximizing resizes at once, and the game must exist to take the resize (inside the
 * window's own constructor it crashes 1.16.5). 26.3: WindowMaximizeMixin.
 */
@Mixin(Minecraft.class)
abstract class MinecraftMaximizeMixin {
	//#if MC < 26.3
	@Inject(method = "<init>", at = @At("TAIL"))
	private void arctic$maximize(CallbackInfo ci) {
		com.mojang.blaze3d.platform.Window window = ((Minecraft) (Object) this).getWindow();
		if (!Boolean.getBoolean("arctic.maximized") || window.isFullscreen()) {
			return;
		}
		//#if MC >= 1.21.9
		org.lwjgl.glfw.GLFW.glfwMaximizeWindow(window.handle());
		//#else
		org.lwjgl.glfw.GLFW.glfwMaximizeWindow(window.getWindow());
		//#endif
	}
	//#endif
}
