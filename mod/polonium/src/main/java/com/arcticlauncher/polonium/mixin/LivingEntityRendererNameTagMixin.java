package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.AttributeValues;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attributes;
import org.spongepowered.asm.mixin.Mixin;

/** The name tag distance attributes, kept per entity as values (see {@link AttributeValues}). */
@Mixin(LivingEntityRenderer.class)
abstract class LivingEntityRendererNameTagMixin {
	@WrapMethod(method = "extractNameTags(Lnet/minecraft/world/entity/LivingEntity;Lnet/minecraft/client/renderer/entity/state/LivingEntityRenderState;F)V")
	private void polonium$keptDistances(LivingEntity entity, LivingEntityRenderState state, float partialTicks, Operation<Void> extract) {
		AttributeValues.Holder2 kept = (AttributeValues.Holder2) entity;
		long version = AttributeValues.version();
		if (kept.polonium$version() != version) {
			// As the game reads them.
			kept.polonium$nameDistances(entity.getAttribute(Attributes.NAME_TAG_DISTANCE).getValue(),
					entity.getAttribute(Attributes.BELOW_NAME_DISTANCE).getValue());
			kept.polonium$version(version);
		}
		((EntityRendererNameTagsAccess) this).polonium$extractNameTags(entity, state, partialTicks, kept.polonium$nameDistance(),
				kept.polonium$belowNameDistance());
	}
}
