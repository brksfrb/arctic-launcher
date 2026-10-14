package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.replay.LegacyReplayPlayback;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Legacy world replacement draws a progress frame directly, outside setScreen. */
@Mixin(net.minecraft.client.render.LoadingScreenRenderer.class)
abstract class LoadingScreenReplayMixin {
	@Inject(method = "setProgressPercentage", at = @At("HEAD"), cancellable = true)
	private void arctic$seekWithoutLoadingFrame(int progress, CallbackInfo ci) {
		if (LegacyReplayPlayback.INSTANCE.seeking()) ci.cancel();
	}
}
