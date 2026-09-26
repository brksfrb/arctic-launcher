package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.Input;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyboardHandler;
//#if MC >= 1.21.9
import net.minecraft.client.input.KeyEvent;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Arctic's keys while playing (Right Shift opens the Arctic menu). */
@Mixin(KeyboardHandler.class)
abstract class KeyboardHandlerMixin {
	//#if MC >= 1.21.9
	@Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
	private void arctic$key(long window, int action, KeyEvent event, CallbackInfo ci) {
		if (handled(event.key(), action)) {
			ci.cancel();
		}
	}
	//#else
	@Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
	private void arctic$key(long window, int key, int scancode, int action, int modifiers, CallbackInfo ci) {
		if (handled(key, action)) {
			ci.cancel();
		}
	}
	//#endif

	private static boolean handled(int key, int action) {
		return action == InputConstants.PRESS && Compat.screen() == null && ArcticClient.keyPressed(Input.key(key));
	}
}
