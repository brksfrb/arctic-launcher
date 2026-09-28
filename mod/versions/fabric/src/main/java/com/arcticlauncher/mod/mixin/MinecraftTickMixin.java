package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Before each client tick: Arctic reads its keys and applies toggles. */
@Mixin(Minecraft.class)
abstract class MinecraftTickMixin {
	@Inject(method = "tick", at = @At("HEAD"))
	private void arctic$tick(CallbackInfo ci) {
		ArcticClient.tick(Compat.screen() != null);
		com.arcticlauncher.mod.DisconnectConfirm.tick();
		//#if MC >= 1.16
		com.arcticlauncher.mod.replay.ReplayRecording.tick();
		//#endif
		//#if MC >= 26.1
		com.arcticlauncher.mod.ArcticPacks.checkFontScale();
		//#endif
	}
}
