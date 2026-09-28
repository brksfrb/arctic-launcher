//#if MC >= 1.20
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Features;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC >= 26.1
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif

/**
 * Freelook (the camera takes its own angles) and Zoom (a narrower field of
 * view). Before 26.1 Camera had no separate alignWithEntity/calculateFov
 * methods: the rotation is set inline in setup, and FOV lives on
 * GameRenderer.getFov (see arctic$zoom below, mixed into GameRenderer).
 */
@Mixin(Camera.class)
abstract class CameraMixin {
	@WrapOperation(
			//#if MC >= 26.1
			method = "alignWithEntity",
			//#else
			method = "setup",
			//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/Entity;getViewYRot(F)F"))
	private float arctic$lookYaw(Entity entity, float partial, Operation<Float> original) {
		Features f = ArcticClient.features();
		return f.freelook() ? f.lookYaw() : original.call(entity, partial);
	}

	@WrapOperation(
			//#if MC >= 26.1
			method = "alignWithEntity",
			//#else
			method = "setup",
			//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/Entity;getViewXRot(F)F"))
	private float arctic$lookPitch(Entity entity, float partial, Operation<Float> original) {
		Features f = ArcticClient.features();
		return f.freelook() ? f.lookPitch() : original.call(entity, partial);
	}

	//#if MC >= 26.1
	@Inject(method = "calculateFov", at = @At("RETURN"), cancellable = true)
	private void arctic$zoom(float partial, CallbackInfoReturnable<Float> cir) {
		float replay = com.arcticlauncher.mod.replay.ReplayView.fov;
		float fov = replay > 0 ? replay : cir.getReturnValue();
		cir.setReturnValue(fov * ArcticClient.features().fovMultiplier());
	}
	//#endif
}
//#endif
