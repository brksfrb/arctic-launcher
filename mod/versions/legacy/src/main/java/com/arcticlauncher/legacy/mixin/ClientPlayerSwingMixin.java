package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.replay.LegacyReplayRecording;
import net.minecraft.entity.player.ClientPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Your arm swings go in the replay (servers never send you your own). */
@Mixin(ClientPlayerEntity.class)
abstract class ClientPlayerSwingMixin {
	//#if MC >= 1.9
	@Inject(method = "swingHand", at = @At("HEAD"))
	private void arctic$swing(net.minecraft.util.Hand hand, CallbackInfo ci) {
		LegacyReplayRecording.swung();
	}
	//#else
	@Inject(method = "swingHand", at = @At("HEAD"))
	private void arctic$swing(CallbackInfo ci) {
		LegacyReplayRecording.swung();
	}
	//#endif
}
