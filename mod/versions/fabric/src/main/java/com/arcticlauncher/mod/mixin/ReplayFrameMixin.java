//#if MC >= 1.13
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.mod.replay.ReplayPlayback;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
//#if MC >= 1.21
import net.minecraft.client.DeltaTracker;
//#endif
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.GameRenderer;
//#if MC >= 26.2
import net.minecraft.client.renderer.extract.LevelExtractor;
//#else
import net.minecraft.client.renderer.LevelRenderer;
//#endif
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A replay moves on before each frame (packets, camera) and grabs it after (video export). */
@Mixin(Minecraft.class)
abstract class ReplayFrameMixin {
	//#if MC >= 26.2
	@WrapOperation(method = "updateLevelInEngines(Lnet/minecraft/client/multiplayer/ClientLevel;Z)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/extract/LevelExtractor;setLevel(Lnet/minecraft/client/multiplayer/ClientLevel;)V"))
	private void arctic$attachReplayRendererNextFrame(LevelExtractor extractor, ClientLevel level, Operation<Void> original) {
		if (!ReplayPlayback.INSTANCE.deferRendererSetup(level, () -> original.call(extractor, level))) original.call(extractor, level);
	}
	//#else
	//#if MC >= 1.21.11
	@WrapOperation(method = "updateLevelInEngines(Lnet/minecraft/client/multiplayer/ClientLevel;Z)V",
	//#elif MC >= 1.14
	@WrapOperation(method = "updateLevelInEngines",
	//#else
	@WrapOperation(method = "method_18206",
	//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/LevelRenderer;setLevel(Lnet/minecraft/client/multiplayer/ClientLevel;)V"))
	private void arctic$attachReplayRendererNextFrame(LevelRenderer renderer, ClientLevel level, Operation<Void> original) {
		if (!ReplayPlayback.INSTANCE.deferRendererSetup(level, () -> original.call(renderer, level))) original.call(renderer, level);
	}
	//#endif

	//#if MC >= 26.1
	@WrapOperation(method = "renderFrame", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/GameRenderer;extract(Lnet/minecraft/client/DeltaTracker;Z)V"))
	//#elif MC >= 1.21
	@WrapOperation(method = "runTick", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/GameRenderer;render(Lnet/minecraft/client/DeltaTracker;Z)V"))
	//#endif
	//#if MC >= 1.21
	private void arctic$drawReplayWorldAfterSetup(GameRenderer renderer, DeltaTracker delta, boolean drawWorld, Operation<Void> original) {
		original.call(renderer, delta, drawWorld && !ReplayPlayback.INSTANCE.worldTransitioning());
	}
	//#else
	@WrapOperation(method = "runTick", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/GameRenderer;render(FJZ)V"))
	private void arctic$drawReplayWorldAfterSetup(GameRenderer renderer, float delta, long time, boolean drawWorld, Operation<Void> original) {
		original.call(renderer, delta, time, drawWorld && !ReplayPlayback.INSTANCE.worldTransitioning());
	}
	//#endif

    //#if MC >= 26.2
    @Inject(method = "exitWorldAndClose", at = @At("HEAD"), require = 1)
    //#else
    @Inject(method = "destroy", at = @At("HEAD"), require = 1)
    //#endif
    private void arctic$stopStarted(CallbackInfo ci) {
        com.arcticlauncher.client.CrashDiagnostics.stopStarted();
    }

    //#if MC >= 26.2
    @Inject(method = "exitWorldAndClose", at = @At("RETURN"), require = 1)
    //#else
    @Inject(method = "destroy", at = @At(value = "INVOKE", target = "Ljava/lang/System;exit(I)V"), require = 1)
    //#endif
    private void arctic$stopComplete(CallbackInfo ci) {
        com.arcticlauncher.client.CrashDiagnostics.stopComplete();
    }

	//#if MC >= 1.16
	// Newer Minecraft forces a render from inside its login/respawn handler.
	//#if MC >= 1.21.9
	@Inject(method = "setScreenAndShow", at = @At("HEAD"), cancellable = true)
	//#else
	@Inject(method = "forceSetScreen", at = @At("HEAD"), cancellable = true)
	//#endif
	private void arctic$replayWithoutForcedFrame(net.minecraft.client.gui.screens.Screen screen, CallbackInfo ci) {
		if (com.arcticlauncher.client.replay.Replays.watching()
				&& com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE.deferForcedFrame(screen)) ci.cancel();
	}
	//#endif

	@Inject(method = "runTick", at = @At("HEAD"))
	private void arctic$replayFrame(boolean advanceGameTime, CallbackInfo ci) {
		//#if DEV_SCENES
		com.arcticlauncher.mod.ReplayTest.frame();
		//#endif
		Replays.frame(com.arcticlauncher.mod.Compat.screen() != null);
		// A game controller moves the player and the pointer each frame.
		com.arcticlauncher.client.ArcticClient.frame(com.arcticlauncher.mod.Compat.screen() != null);
	}

	//#if MC >= 26.1
	@Inject(method = "runTick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Minecraft;renderFrame(Z)V", shift = At.Shift.AFTER))
	private void arctic$replayFrameDone(boolean advanceGameTime, CallbackInfo ci) {
		Replays.afterFrame();
	}
	//#endif
}
//#endif
