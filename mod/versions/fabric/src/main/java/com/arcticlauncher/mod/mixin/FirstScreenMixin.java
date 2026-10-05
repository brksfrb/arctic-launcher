package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The first screen the game opens is the title screen (Arctic's or the game's own): the start is done. */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Gui.class)
//#else
@Mixin(net.minecraft.client.Minecraft.class)
//#endif
abstract class FirstScreenMixin {
	@Inject(method = "setScreen", at = @At("HEAD"))
	private void arctic$firstScreen(Screen screen, CallbackInfo ci) {
		if (screen != null) {
			Timeline.title();
		}
	}
}
