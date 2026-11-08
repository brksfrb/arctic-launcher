package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Name tags, item frames and the like turn to the dispatcher's yaw/pitch,
 * taken from the camera entity when the frame's entities start. While
 * Freelook is on, lend it Freelook's angles for that moment.
 */
@Mixin(EntityRenderDispatcher.class)
abstract class FreelookBillboardDispatcherMixin {
	private static final String UPDATE = "updateCamera(Lnet/minecraft/world/World;Lnet/minecraft/client/font/TextRenderer;"
			+ "Lnet/minecraft/entity/Entity;Lnet/minecraft/entity/Entity;Lnet/minecraft/client/option/GameOptions;F)V";

	@Inject(method = UPDATE, at = @At("HEAD"))
	private void arctic$freelookStart(CallbackInfo ci) {
		LegacyHooks.freelookCamera(true);
	}

	@Inject(method = UPDATE, at = @At("RETURN"))
	private void arctic$freelookEnd(CallbackInfo ci) {
		LegacyHooks.freelookCamera(false);
	}
}
