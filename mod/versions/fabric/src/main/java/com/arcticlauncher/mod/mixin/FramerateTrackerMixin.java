package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.2
import com.arcticlauncher.client.feature.DynamicFps;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Dynamic FPS from 1.21.2, where the game keeps its frame rate limit in a tracker of its own (it already
 * slows down when minimized or away): the limit it gives is lowered further while the game isn't in use.
 * Named by text, so a version that moved the class simply goes without.
 */
@Mixin(targets = "com.mojang.blaze3d.platform.FramerateLimitTracker")
abstract class FramerateTrackerMixin {
	@Inject(method = "getFramerateLimit", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$dynamic(CallbackInfoReturnable<Integer> cir) {
		cir.setReturnValue(DynamicFps.limit(cir.getReturnValue()));
	}
}
//#endif
