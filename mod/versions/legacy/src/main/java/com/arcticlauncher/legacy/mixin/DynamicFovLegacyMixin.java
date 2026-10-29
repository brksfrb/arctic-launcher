package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.entity.attribute.EntityAttributeInstance;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Dynamic FOV (1.8.9 - 1.12.2): the view's change with speed and with a drawn bow, each switched off on its own.
 * The game's number is flying (x1.1), the movement speed against the walking speed, and the bow (x1 - 0.15 at
 * most): the speed is read as the walking speed (no change) and the constants are neutral.
 */
@Mixin(AbstractClientPlayerEntity.class)
abstract class DynamicFovLegacyMixin {
	@Redirect(method = "getSpeed", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/entity/attribute/EntityAttributeInstance;getValue()D"), require = 0)
	private double arctic$speed(EntityAttributeInstance attribute) {
		ClientConfig c = ArcticClient.config();
		if (c != null && c.fovNoSpeed) {
			return ((AbstractClientPlayerEntity) (Object) this).abilities.getWalkSpeed();
		}
		return attribute.getValue();
	}

	@ModifyConstant(method = "getSpeed", constant = @Constant(floatValue = 1.1f), require = 0)
	private float arctic$flying(float value) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoSpeed ? 1f : value;
	}

	@ModifyConstant(method = "getSpeed", constant = @Constant(floatValue = 0.15f), require = 0)
	private float arctic$bow(float value) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoBow ? 0f : value;
	}
}
