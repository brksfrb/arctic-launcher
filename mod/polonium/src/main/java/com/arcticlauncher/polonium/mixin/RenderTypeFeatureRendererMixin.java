//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.GpuBatches;
import com.arcticlauncher.polonium.gpu.GpuBatchesHolder;
import java.util.List;
import net.minecraft.client.renderer.feature.FeatureFrameContext;
import net.minecraft.client.renderer.feature.RenderTypeFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The group bookkeeping for {@link GpuBatches}: which group the model and
 * item renderers are preparing, drawing its GPU batches when the game draws that
 * group (before the game's own draws, so effects like enchantment glint that
 * go on top still do), and starting over each frame.
 */
@Mixin(RenderTypeFeatureRenderer.class)
abstract class RenderTypeFeatureRendererMixin {
	@Inject(method = "prepareGroup", at = @At("HEAD"))
	private void polonium$beginGroup(FeatureFrameContext context, List<?> submits, boolean strictlyOrdered, CallbackInfo ci) {
		if (this instanceof GpuBatchesHolder holder) {
			GpuBatches.announce();
			holder.polonium$batches().beginGroup(strictlyOrdered);
		}
	}

	@Inject(method = "prepareGroup", at = @At("TAIL"))
	private void polonium$endGroup(FeatureFrameContext context, List<?> submits, boolean strictlyOrdered, CallbackInfo ci) {
		if (this instanceof GpuBatchesHolder holder) {
			holder.polonium$batches().endGroup();
		}
	}

	@Inject(method = "executeGroup", at = @At("HEAD"))
	private void polonium$drawGroup(FeatureFrameContext context, int groupIndex, List<?> submits, boolean strictlyOrdered, CallbackInfo ci) {
		if (this instanceof GpuBatchesHolder holder) {
			holder.polonium$batches().executeGroup(groupIndex);
		}
	}

	@Inject(method = "finishExecute", at = @At("HEAD"))
	private void polonium$endFrame(FeatureFrameContext context, CallbackInfo ci) {
		if (this instanceof GpuBatchesHolder holder) {
			holder.polonium$batches().endFrame();
		}
	}
}
//#endif
