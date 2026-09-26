package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Features;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Freelook (the camera takes its own angles) and Zoom (a narrower field of
 * view). Listed in the mixin config only where these targets exist (26.3).
 */
@Mixin(Camera.class)
abstract class CameraMixin {
	@WrapOperation(method = "alignWithEntity", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/Entity;getViewYRot(F)F"))
	private float arctic$lookYaw(Entity entity, float partial, Operation<Float> original) {
		Features f = ArcticClient.features();
		return f.freelook() ? f.lookYaw() : original.call(entity, partial);
	}

	@WrapOperation(method = "alignWithEntity", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/Entity;getViewXRot(F)F"))
	private float arctic$lookPitch(Entity entity, float partial, Operation<Float> original) {
		Features f = ArcticClient.features();
		return f.freelook() ? f.lookPitch() : original.call(entity, partial);
	}

	@Inject(method = "calculateFov", at = @At("RETURN"), cancellable = true)
	private void arctic$zoom(float partial, CallbackInfoReturnable<Float> cir) {
		cir.setReturnValue(cir.getReturnValue() * ArcticClient.features().fovMultiplier());
	}
}
