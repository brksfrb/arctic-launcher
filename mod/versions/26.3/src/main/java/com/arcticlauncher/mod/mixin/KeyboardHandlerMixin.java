package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.input.KeyEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Arctic's keys while playing (Right Shift opens the Arctic menu). */
@Mixin(KeyboardHandler.class)
abstract class KeyboardHandlerMixin {
	/** GLFW_PRESS. */
	private static final int PRESS = 1;

	@Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
	private void arctic$key(long window, int action, KeyEvent event, CallbackInfo ci) {
		if (action == PRESS
				&& Minecraft.getInstance().gui.screen() == null
				&& ArcticClient.keyPressed(event.key())) {
			ci.cancel();
		}
	}
}
