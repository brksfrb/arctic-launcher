package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.Fog;
import com.mojang.blaze3d.platform.GlStateManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Fog: the distance where the (linear) fog starts and ends is stretched by the player's factor (or pushed out of
 * sight). The value the game is about to set is scaled, never the GL state read back, so nothing compounds, Fog
 * off is vanilla again the next frame, and the game's own cached fog state stays the truth (writing GL directly
 * behind its back left the cache saying "already 128" while GL kept the old value, so it never got reset).
 */
@Mixin(GlStateManager.class)
abstract class FogLegacyMixin {
	@ModifyVariable(method = "fogStart", at = @At("HEAD"), argsOnly = true)
	private static float arctic$start(float start) {
		return Fog.stretch(start);
	}

	@ModifyVariable(method = "fogEnd", at = @At("HEAD"), argsOnly = true)
	private static float arctic$end(float end) {
		return Fog.stretch(end);
	}
}
