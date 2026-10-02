//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelExtract;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The level's entity states made on the helpers during the frame's extraction are in before the frame is drawn. */
@Mixin(GameRenderer.class)
abstract class GameRendererExtractMixin {
	@Inject(method = "extract", at = @At("TAIL"))
	private void polonium$entitiesIn(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		ParallelExtract.finish();
	}

	@Inject(method = "render", at = @At("HEAD"))
	private void polonium$entitiesInBeforeDrawing(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		ParallelExtract.finish();
	}
}
//#endif
