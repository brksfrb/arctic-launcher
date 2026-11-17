package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.render.Camera;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Particles face along the rotation vectors Camera.update works out from the
 * camera entity's angles. That runs after the camera setup has put the real
 * angles back, so while Freelook is on, lend it Freelook's angles again.
 */
@Mixin(Camera.class)
abstract class FreelookBillboardCameraMixin {
	@Inject(method = "update", at = @At("HEAD"))
	private static void arctic$freelookStart(PlayerEntity player, boolean front, CallbackInfo ci) {
		LegacyHooks.freelookCamera(true);
	}

	@Inject(method = "update", at = @At("RETURN"))
	private static void arctic$freelookEnd(PlayerEntity player, boolean front, CallbackInfo ci) {
		LegacyHooks.freelookCamera(false);
	}
}
