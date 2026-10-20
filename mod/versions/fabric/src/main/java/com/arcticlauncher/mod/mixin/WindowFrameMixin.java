//#if MC < 26.1
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.mojang.blaze3d.platform.Window;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Before 26.1: the frame is complete in the main render target when the
 * window is about to show it (hooked here, not at the call in runTick,
 * which performance mods rewrite).
 */
@Mixin(Window.class)
abstract class WindowFrameMixin {
	//#if MC < 1.14
	// 1.13: the swap and event poll are in method_18301(boolean).
	@Inject(method = "method_18301", at = @At("HEAD"))
	private void arctic$replayFrameDone(boolean flag, CallbackInfo ci) {
	//#else
	@Inject(method = "updateDisplay", at = @At("HEAD"))
	private void arctic$replayFrameDone(CallbackInfo ci) {
	//#endif
		Replays.afterFrame();
	}
}
//#endif
