//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.Timeline;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Marks when the game's window exists. */
@Mixin(targets = "com.mojang.blaze3d.platform.Window")
abstract class WindowMarkMixin {
	@Inject(method = "<init>", at = @At("TAIL"), require = 0)
	private void arctic$window(CallbackInfo ci) {
		Timeline.mark("window");
	}
}
//#endif
