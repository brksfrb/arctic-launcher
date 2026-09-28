package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Zoom narrows the view; Freelook turns the camera, not the player. */
@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
	@Inject(method = "getFov", at = @At("RETURN"), cancellable = true)
	private void arctic$zoom(float tickDelta, boolean changing, CallbackInfoReturnable<Float> cir) {
		float replay = com.arcticlauncher.legacy.replay.LegacyReplayView.fov;
		if (replay > 0 && changing) {
			cir.setReturnValue(replay);
		}
		if (ArcticClient.features() != null) {
			cir.setReturnValue(cir.getReturnValue() * ArcticClient.features().fovMultiplier());
		}
		if (changing) {
			LegacyHooks.worldFov = cir.getReturnValue();
		}
	}

	@Inject(method = "transformCamera", at = @At("HEAD"))
	private void arctic$freelookStart(float tickDelta, CallbackInfo ci) {
		LegacyHooks.freelookCamera(true);
		LegacyHooks.noteCamera(tickDelta);
	}

	@Inject(method = "transformCamera", at = @At("RETURN"))
	private void arctic$freelookEnd(float tickDelta, CallbackInfo ci) {
		LegacyHooks.freelookCamera(false);
	}
}
