//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Marks when the game object (and its window) exists. */
@Mixin(Minecraft.class)
abstract class MinecraftReadyMixin {
	@Inject(method = "<init>", at = @At("TAIL"))
	private void arctic$ready(CallbackInfo ci) {
		Timeline.mark("minecraft-ready");
	}
}
//#endif
