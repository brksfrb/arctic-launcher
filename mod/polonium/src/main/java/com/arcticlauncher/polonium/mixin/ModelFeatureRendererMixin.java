package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.GpuEntities;
import com.arcticlauncher.polonium.gpu.GpuBatches;
import com.arcticlauncher.polonium.gpu.GpuBatchesHolder;
import com.arcticlauncher.polonium.gpu.GpuEntitiesHolder;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;

/** Entity models go to the GPU path instead of being turned into vertices (see {@link GpuEntities}). */
@Mixin(ModelFeatureRenderer.class)
abstract class ModelFeatureRendererMixin implements GpuBatchesHolder, GpuEntitiesHolder {
	@Unique
	private final GpuEntities polonium$gpu = new GpuEntities();

	@Override
	public GpuBatches polonium$batches() {
		return polonium$gpu.batches();
	}

	@Override
	public GpuEntities polonium$entities() {
		return polonium$gpu;
	}

	@WrapOperation(
			method = "buildGroup",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/feature/ModelFeatureRenderer;prepareModel(Lnet/minecraft/client/renderer/feature/ModelFeatureRenderer$Submit;)V"))
	private void polonium$onGpu(ModelFeatureRenderer self, ModelFeatureRenderer.Submit<?> submit, Operation<Void> build) {
		if (!polonium$gpu.capture(submit)) {
			build.call(self, submit);
		}
	}
}
