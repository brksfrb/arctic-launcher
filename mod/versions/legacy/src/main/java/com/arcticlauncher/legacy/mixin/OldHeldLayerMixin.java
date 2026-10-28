//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.render.entity.feature.HeldItemRenderer;
import net.minecraft.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Marks the held-item layer that people and mobs wear (not a witch's potion, which sets its own turn): 1.7's
 * way of holding an item in third person (OldItemPositionMixin) is only for what this layer draws.
 */
@Mixin(HeldItemRenderer.class)
abstract class OldHeldLayerMixin {
	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$begin(LivingEntity entity, float a, float b, float c, float d, float e, float f, float g, CallbackInfo ci) {
		LegacyHooks.heldLayerActive = true;
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(LivingEntity entity, float a, float b, float c, float d, float e, float f, float g, CallbackInfo ci) {
		LegacyHooks.heldLayerActive = false;
	}
}
//#endif
