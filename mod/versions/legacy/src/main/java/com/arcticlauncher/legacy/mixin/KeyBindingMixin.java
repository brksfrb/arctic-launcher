package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyKeys;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Every key and mouse press while playing (no screen) passes here (mouse
 * buttons as -100 + n): clicks count for CPS, Right Shift opens the menu.
 */
@Mixin(KeyBinding.class)
abstract class KeyBindingMixin {
	private static final int MOUSE_OFFSET = -100;

	@Inject(method = "onKeyPressed", at = @At("HEAD"), cancellable = true)
	private static void arctic$press(int code, CallbackInfo ci) {
		if (code < 0) {
			ArcticClient.mousePressed(LegacyKeys.coreMouse(code - MOUSE_OFFSET));
			return;
		}
		if (MinecraftClient.getInstance().currentScreen == null && ArcticClient.keyPressed(LegacyKeys.core(code))) {
			ci.cancel();
		}
	}
}
