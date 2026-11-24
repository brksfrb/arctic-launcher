package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Legacy world replacement draws a progress frame directly, outside setScreen. */
@Mixin(net.minecraft.client.render.LoadingScreenRenderer.class)
abstract class LoadingScreenReplayMixin {
	@Inject(method = "setProgressPercentage", at = @At("HEAD"), cancellable = true)
	private void arctic$replayWithoutLoadingFrame(int progress, CallbackInfo ci) {
		if (Replays.watching()) ci.cancel();
	}
}
