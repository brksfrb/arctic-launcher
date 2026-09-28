//#if MC >= 1.16
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A replay moves on before each frame (packets, camera) and grabs it after (video export). */
@Mixin(Minecraft.class)
abstract class ReplayFrameMixin {
	@Inject(method = "runTick", at = @At("HEAD"))
	private void arctic$replayFrame(boolean advanceGameTime, CallbackInfo ci) {
		Replays.frame();
	}

	//#if MC >= 26.1
	@Inject(method = "runTick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Minecraft;renderFrame(Z)V", shift = At.Shift.AFTER))
	private void arctic$replayFrameDone(boolean advanceGameTime, CallbackInfo ci) {
		Replays.afterFrame();
	}
	//#endif
}
//#endif
