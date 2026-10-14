package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Each frame: queued game-thread work runs, a replay moves on, and the frame
 * is grabbed (for video) just before it's shown.
 */
@Mixin(MinecraftClient.class)
abstract class MinecraftClientReplayMixin {
	@Inject(method = "runGameLoop", at = @At("HEAD"))
	private void arctic$replayFrame(CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyHooks.runQueued();
		Replays.frame(((MinecraftClient) (Object) this).currentScreen != null);
		// A game controller moves the player and the pointer each frame.
		com.arcticlauncher.client.ArcticClient.frame(((net.minecraft.client.MinecraftClient) (Object) this).currentScreen != null);
	}

	@Inject(method = "updateDisplay", at = @At("HEAD"))
	private void arctic$replayFrameDone(CallbackInfo ci) {
		Replays.afterFrame();
	}
}
