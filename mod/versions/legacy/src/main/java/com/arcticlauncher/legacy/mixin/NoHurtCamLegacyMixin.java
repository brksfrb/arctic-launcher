package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** No Hurt Shake: the view doesn't tilt and shake when you take damage. */
@Mixin(GameRenderer.class)
abstract class NoHurtCamLegacyMixin {
	@Inject(method = "bobViewWhenHurt", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$noShake(float partialTicks, CallbackInfo ci) {
		if (ArcticClient.config() != null && ArcticClient.config().noHurtCam) {
			ci.cancel();
		}
	}
}
