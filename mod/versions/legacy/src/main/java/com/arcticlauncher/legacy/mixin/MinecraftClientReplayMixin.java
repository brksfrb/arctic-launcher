package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.legacy.replay.LegacyReplayPlayback;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.world.ClientWorld;
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
	@WrapOperation(method = "connect(Lnet/minecraft/client/world/ClientWorld;Ljava/lang/String;)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/WorldRenderer;setWorld(Lnet/minecraft/client/world/ClientWorld;)V"))
	private void arctic$attachReplayRendererNextFrame(WorldRenderer renderer, ClientWorld world, Operation<Void> original) {
		if (!LegacyReplayPlayback.INSTANCE.deferRendererSetup(world, () -> original.call(renderer, world))) original.call(renderer, world);
	}

	@WrapOperation(method = "connect(Lnet/minecraft/client/world/ClientWorld;Ljava/lang/String;)V",
			at = @At(value = "INVOKE", target = "Ljava/lang/System;gc()V"))
	private void arctic$skipReplayJoinCollection(Operation<Void> original, ClientWorld world, String message) {
		// Vanilla requests a full collection even when joining from the title screen.
		if (world == null || !LegacyReplayPlayback.INSTANCE.worldTransitioning()) original.call();
	}

    @Inject(method = "stop", at = @At("HEAD"), require = 0)
    private void arctic$stopStarted(CallbackInfo ci) {
        com.arcticlauncher.client.CrashDiagnostics.stopStarted();
    }

    @Inject(method = "stop", at = @At(value = "INVOKE", target = "Ljava/lang/System;exit(I)V"), require = 0)
    private void arctic$stopComplete(CallbackInfo ci) {
        com.arcticlauncher.client.CrashDiagnostics.stopComplete();
    }

	@Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
	private void arctic$keepReplayView(net.minecraft.client.gui.screen.Screen screen, CallbackInfo ci) {
		if (com.arcticlauncher.legacy.replay.LegacyReplayPlayback.INSTANCE.suppressSeekScreen(screen)) ci.cancel();
	}

	@Inject(method = "runGameLoop", at = @At("HEAD"))
	private void arctic$replayFrame(CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyHooks.runQueued();
		com.arcticlauncher.legacy.LegacyGlintTint.tick();
		Replays.frame(((MinecraftClient) (Object) this).currentScreen != null);
		// A game controller moves the player and the pointer each frame.
		com.arcticlauncher.client.ArcticClient.frame(((net.minecraft.client.MinecraftClient) (Object) this).currentScreen != null);
	}

	@Inject(method = "updateDisplay", at = @At("HEAD"))
	private void arctic$replayFrameDone(CallbackInfo ci) {
		Replays.afterFrame();
	}
}
