package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** While freelooking, mouse movement turns the camera instead of you. */
@Mixin(Entity.class)
abstract class EntityMixin {
	@Inject(method = "increaseTransforms", at = @At("HEAD"), cancellable = true)
	private void arctic$turn(float yaw, float pitch, CallbackInfo ci) {
		if ((Object) this == MinecraftClient.getInstance().player && ArcticClient.features() != null
				// 1.8.9 subtracts the pitch change (its mouse Y points up); newer
				// versions add it, which is what the shared freelook expects.
				&& ArcticClient.features().turn(yaw, -pitch)) {
			ci.cancel();
		}
	}
}
