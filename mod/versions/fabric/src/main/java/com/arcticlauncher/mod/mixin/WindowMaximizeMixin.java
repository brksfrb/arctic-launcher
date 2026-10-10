package com.arcticlauncher.mod.mixin;

import com.mojang.blaze3d.platform.Window;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;

/**
 * "Start the game maximized" (the launcher sets arctic.maximized) on 26.3, whose window is SDL's;
 * earlier versions: MinecraftMaximizeMixin. The game maximizes its own window, the one way that
 * works on every system (Wayland lets no other program do it).
 */
@Mixin(Window.class)
abstract class WindowMaximizeMixin {
	//#if MC >= 26.3
	/** Done once: a player who restores the window keeps it that way. */
	@org.spongepowered.asm.mixin.Unique
	private static boolean arctic$maximized;

	// 26.3 opens its window through SDL and sizes it once more after making it: maximized after that.
	@Inject(method = "applyWindowed", at = @At("RETURN"))
	private void arctic$maximize(org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Boolean> cir) {
		if (arctic$maximized || !Boolean.getBoolean("arctic.maximized")) {
			return;
		}
		arctic$maximized = true;
		org.lwjgl.sdl.SDLVideo.SDL_MaximizeWindow(((Window) (Object) this).handle());
	}
	//#endif
}
