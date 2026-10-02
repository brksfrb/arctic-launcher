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
	@Inject(method = "extract", at = @At("HEAD"))
	private void polonium$extractStarts(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.EXTRACT);
	}

	@Inject(method = "extract", at = @At("TAIL"))
	private void polonium$entitiesIn(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.EXTRACT_JOIN);
		ParallelExtract.finish();
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.EXTRACT_JOIN);
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.EXTRACT);
	}

	@Inject(method = "render", at = @At("HEAD"))
	private void polonium$entitiesInBeforeDrawing(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		ParallelExtract.finish();
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.RENDER);
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void polonium$frameDrawn(DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.RENDER);
		com.arcticlauncher.polonium.Timeline.frame();
	}
}
//#endif
