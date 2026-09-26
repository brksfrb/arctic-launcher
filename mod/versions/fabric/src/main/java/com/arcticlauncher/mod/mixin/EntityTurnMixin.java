package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Mouse look for the local player: while Freelook is held the camera turns
 * instead of the player, and while zoomed the mouse is slower.
 */
@Mixin(Entity.class)
abstract class EntityTurnMixin {
	@Inject(method = "turn", at = @At("HEAD"), cancellable = true)
	private void arctic$freelook(double yaw, double pitch, CallbackInfo ci) {
		if (isLocalPlayer() && ArcticClient.features().turn(yaw, pitch)) {
			ci.cancel();
		}
	}

	@ModifyVariable(method = "turn", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private double arctic$zoomYaw(double yaw) {
		return isLocalPlayer() ? yaw * ArcticClient.features().sensitivityMultiplier() : yaw;
	}

	@ModifyVariable(method = "turn", at = @At("HEAD"), argsOnly = true, ordinal = 1)
	private double arctic$zoomPitch(double pitch) {
		return isLocalPlayer() ? pitch * ArcticClient.features().sensitivityMultiplier() : pitch;
	}

	private boolean isLocalPlayer() {
		return (Object) this == Minecraft.getInstance().player;
	}
}
