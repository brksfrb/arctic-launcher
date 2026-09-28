package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Arctic's HUD widgets (and its crosshair, instead of the game's when on). */
@Mixin(InGameHud.class)
abstract class InGameHudMixin {
	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$hud(float tickDelta, CallbackInfo ci) {
		LegacyHooks.renderHud();
	}

	//#if MC >= 1.9
	/** 1.9+ draws the crosshair (and the attack indicator under it) here. */
	@Inject(method = "method_12164", at = @At("HEAD"), cancellable = true)
	private void arctic$crosshair(CallbackInfo ci) {
		if (LegacyHooks.ownCrosshair()) {
			ci.cancel();
		}
	}
	//#else
	@Inject(method = "showCrosshair", at = @At("RETURN"), cancellable = true)
	private void arctic$crosshair(CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValue() && LegacyHooks.ownCrosshair()) {
			cir.setReturnValue(false);
		}
	}
	//#endif
}
