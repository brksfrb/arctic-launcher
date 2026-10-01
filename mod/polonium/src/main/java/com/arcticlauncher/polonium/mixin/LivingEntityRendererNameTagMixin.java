package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.AttributeCache;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.core.Holder;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** The name tag distance attributes, remembered per entity (see {@link AttributeCache}). */
@Mixin(LivingEntityRenderer.class)
abstract class LivingEntityRendererNameTagMixin {
	@WrapOperation(method = "extractNameTags",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/LivingEntity;getAttribute(Lnet/minecraft/core/Holder;)Lnet/minecraft/world/entity/ai/attributes/AttributeInstance;"))
	private AttributeInstance polonium$rememberedAttribute(LivingEntity entity, Holder<Attribute> attribute, Operation<AttributeInstance> lookUp) {
		return AttributeCache.get(entity, attribute, () -> lookUp.call(entity, attribute));
	}
}
