//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Marks when the game's own main() begins (before its registries are built). */
@Mixin(targets = "net.minecraft.client.main.Main")
abstract class MainStartMixin {
	@Inject(method = "main", at = @At("HEAD"))
	private static void arctic$main(String[] args, CallbackInfo ci) {
		Timeline.mark("main");
	}
}
//#endif
