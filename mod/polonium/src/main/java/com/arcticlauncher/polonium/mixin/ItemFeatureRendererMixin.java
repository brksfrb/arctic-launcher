//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.GpuBatches;
import com.arcticlauncher.polonium.gpu.GpuBatchesHolder;
import com.arcticlauncher.polonium.gpu.GpuItems;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.renderer.feature.ItemFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;

/** Items go to the GPU path instead of being turned into vertices (see {@link GpuItems}); glint stays as it is. */
@Mixin(ItemFeatureRenderer.class)
abstract class ItemFeatureRendererMixin implements GpuBatchesHolder {
	@Unique
	private final GpuItems polonium$gpu = new GpuItems();

	@Override
	public GpuBatches polonium$batches() {
		return polonium$gpu.batches();
	}

	@WrapOperation(
			method = "prepareSubmit",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/feature/ItemFeatureRenderer;prepareMainSubmit(Lnet/minecraft/client/renderer/feature/ItemFeatureRenderer$Submit;)V"))
	private void polonium$onGpu(ItemFeatureRenderer self, ItemFeatureRenderer.Submit submit, Operation<Void> build) {
		if (!polonium$gpu.capture(submit)) {
			build.call(self, submit);
		}
	}
}
//#endif
