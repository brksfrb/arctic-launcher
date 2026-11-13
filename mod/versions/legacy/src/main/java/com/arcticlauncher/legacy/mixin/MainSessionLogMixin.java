package com.arcticlauncher.legacy.mixin;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Before the game logs anything: its "(Session ID is token:...)" line is written without the access token. */
@Mixin(targets = "net.minecraft.client.main.Main")
abstract class MainSessionLogMixin {
	@Inject(method = "main", at = @At("HEAD"), require = 0)
	private static void arctic$maskSession(String[] args, CallbackInfo ci) {
		com.arcticlauncher.legacy.SessionLogMask.install();
	}
}
