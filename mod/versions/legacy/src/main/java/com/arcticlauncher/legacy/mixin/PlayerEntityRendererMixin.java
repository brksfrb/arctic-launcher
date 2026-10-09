package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyCosmeticLayer;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Players get Arctic's cosmetics layer. */
@Mixin(PlayerEntityRenderer.class)
abstract class PlayerEntityRendererMixin {
	@Inject(method = "<init>(Lnet/minecraft/client/render/entity/EntityRenderDispatcher;Z)V", at = @At("TAIL"))
	private void arctic$cosmetics(EntityRenderDispatcher dispatcher, boolean slim, CallbackInfo ci) {
		// Through an invoker: a @Shadow of the inherited addFeature isn't found on the player
		// renderer at runtime, and the game didn't start.
		((LivingEntityFeatureAccess) this).arctic$addFeature(new LegacyCosmeticLayer((PlayerEntityRenderer) (Object) this));
	}
}
