package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacySmoothFont;
import com.mojang.blaze3d.platform.GlStateManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Follows how much the game has scaled what it draws (a title is drawn 4 times as big), so the pixel-sharp font
 * can draw big text from smooth letters instead of magnifying its pixels.
 */
@Mixin(GlStateManager.class)
abstract class GlScaleMixin {
	@Inject(method = "pushMatrix", at = @At("HEAD"), require = 0)
	private static void arctic$push(CallbackInfo ci) {
		LegacySmoothFont.matrixPush();
	}

	@Inject(method = "popMatrix", at = @At("HEAD"), require = 0)
	private static void arctic$pop(CallbackInfo ci) {
		LegacySmoothFont.matrixPop();
	}

	@Inject(method = "loadIdentity", at = @At("HEAD"), require = 0)
	private static void arctic$identity(CallbackInfo ci) {
		LegacySmoothFont.matrixIdentity();
	}

	@Inject(method = "scale(FFF)V", at = @At("HEAD"), require = 0)
	private static void arctic$scale(float x, float y, float z, CallbackInfo ci) {
		LegacySmoothFont.matrixScale(x);
	}

	@Inject(method = "scale(DDD)V", at = @At("HEAD"), require = 0)
	private static void arctic$scaleDouble(double x, double y, double z, CallbackInfo ci) {
		LegacySmoothFont.matrixScale((float) x);
	}
}
