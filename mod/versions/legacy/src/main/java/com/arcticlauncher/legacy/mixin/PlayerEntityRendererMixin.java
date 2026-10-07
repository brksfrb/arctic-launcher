package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyCosmeticLayer;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Players get Arctic's cosmetics layer. */
@Mixin(PlayerEntityRenderer.class)
abstract class PlayerEntityRendererMixin {
	@Shadow
	@SuppressWarnings("rawtypes")
	protected abstract boolean addFeature(FeatureRenderer feature);

	@Inject(method = "<init>(Lnet/minecraft/client/render/entity/EntityRenderDispatcher;Z)V", at = @At("TAIL"))
	private void arctic$cosmetics(EntityRenderDispatcher dispatcher, boolean slim, CallbackInfo ci) {
		addFeature(new LegacyCosmeticLayer((PlayerEntityRenderer) (Object) this));
	}
}
