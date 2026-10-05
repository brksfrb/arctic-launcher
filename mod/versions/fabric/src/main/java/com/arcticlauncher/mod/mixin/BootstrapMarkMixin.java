//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Marks when the game's registries (blocks, items, entities...) are built. */
@Mixin(targets = "net.minecraft.server.Bootstrap")
abstract class BootstrapMarkMixin {
	//#if MC >= 26.1
	/** The registries (and with them every block state) exist: make the block states' caches now. */
	@Inject(method = "bootStrap", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/FireBlock;bootStrap()V"))
	private static void arctic$caches(CallbackInfo ci) {
		com.arcticlauncher.mod.startup.StateCaches.runAll();
	}
	//#endif

	@Inject(method = "bootStrap", at = @At("TAIL"))
	private static void arctic$bootstrapped(CallbackInfo ci) {
		Timeline.mark("bootstrap");
	}
}
//#endif
