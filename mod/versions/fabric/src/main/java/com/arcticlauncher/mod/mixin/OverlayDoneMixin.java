//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import net.minecraft.client.gui.screens.Overlay;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The loading screen is gone: the game is ready (the title screen is shown, and usable). */
//#if MC >= 26.2
@Mixin(targets = "net.minecraft.client.gui.Gui")
//#else
@Mixin(targets = "net.minecraft.client.Minecraft")
//#endif
abstract class OverlayDoneMixin {
	@Inject(method = "setOverlay", at = @At("HEAD"), require = 0)
	private void arctic$overlay(Overlay overlay, CallbackInfo ci) {
		if (overlay == null) {
			Timeline.ready();
		}
	}
}
//#endif
