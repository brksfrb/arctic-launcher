package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ServerMenus;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The game's own multiplayer and disconnect screens never get a frame: Arctic's takes their place
 * as they open (the per-tick swap alone showed the native one for a moment first).
 */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Gui.class)
//#else
@Mixin(net.minecraft.client.Minecraft.class)
//#endif
abstract class ServerMenuScreenMixin {
	@Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
	private void arctic$serverMenus(Screen screen, CallbackInfo ci) {
		if (ServerMenus.intercept(screen)) {
			ci.cancel();
		}
	}
}
