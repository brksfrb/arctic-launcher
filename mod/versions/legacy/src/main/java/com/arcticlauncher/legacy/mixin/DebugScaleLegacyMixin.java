package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DebugStyle;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.gui.hud.DebugHud;
import net.minecraft.client.util.Window;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Debug Screen text size: each column of F3 is drawn inside a scale that keeps it in its corner. */
@Mixin(DebugHud.class)
abstract class DebugScaleLegacyMixin {
	private static boolean arctic$scaled;

	private static void arctic$begin(float shiftWidth) {
		float factor = DebugStyle.scale(ArcticClient.config());
		arctic$scaled = factor != 1f;
		if (arctic$scaled) {
			GlStateManager.pushMatrix();
			GlStateManager.translate(shiftWidth * (1f - factor), 0f, 0f);
			GlStateManager.scale(factor, factor, 1f);
		}
	}

	private static void arctic$finish() {
		if (arctic$scaled) {
			arctic$scaled = false;
			GlStateManager.popMatrix();
		}
	}

	@Inject(method = "renderLeftText", at = @At("HEAD"), require = 0)
	private void arctic$leftStart(CallbackInfo ci) {
		arctic$begin(0f);
	}

	@Inject(method = "renderLeftText", at = @At("RETURN"), require = 0)
	private void arctic$leftEnd(CallbackInfo ci) {
		arctic$finish();
	}

	@Inject(method = "renderRightText", at = @At("HEAD"), require = 0)
	private void arctic$rightStart(Window window, CallbackInfo ci) {
		arctic$begin(window.getWidth());
	}

	@Inject(method = "renderRightText", at = @At("RETURN"), require = 0)
	private void arctic$rightEnd(Window window, CallbackInfo ci) {
		arctic$finish();
	}
}
