package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Light Overlay on 1.8.9 - 1.12.2: the markers drawn in the world after the terrain and outline, just before the weather. */
@Mixin(GameRenderer.class)
abstract class LightOverlayLegacyMixin {
	@Inject(method = "renderWorld(IFJ)V", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/GameRenderer;renderWeather(F)V"), require = 0)
	private void arctic$lightOverlay(int pass, float tickDelta, long nanoTime, CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyLightOverlay.draw(tickDelta);
	}
}
