package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTagCache;
import com.arcticlauncher.polonium.gpu.GpuEntitiesHolder;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.feature.FeatureFrameContext;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import net.minecraft.client.renderer.feature.FeatureRendererMap;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Once the frame's submits are sorted, before any is prepared: a fresh name tag cache, and the GPU path looks them over. */
@Mixin(FeatureRenderDispatcher.class)
abstract class FeatureRenderDispatcherMixin {
	@Shadow
	@Final
	private FeatureRendererMap featureRenderers;

	@Inject(method = "prepareFrameWithContext",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/util/profiling/ProfilerFiller;popPush(Ljava/lang/String;)V", ordinal = 0))
	private void polonium$scan(FeatureFrameContext context, SubmitNodeStorage storage,
			CallbackInfoReturnable<FeatureRenderDispatcher.PreparedFrame> cir, @Local FeatureRenderDispatcher.PreparedFrame frame) {
		NameTagCache.newFrame();
		if (featureRenderers.get(ModelFeatureRenderer.TYPE) instanceof GpuEntitiesHolder holder) {
			holder.polonium$entities().scan(((PreparedFrameAccess) frame).polonium$allSubmits());
		}
	}
}
