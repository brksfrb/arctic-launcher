package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The client tick: Arctic reads its held keys (Zoom, Freelook). */
@Mixin(Minecraft.class)
abstract class MinecraftTickMixin {
	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$tick(CallbackInfo ci) {
		ArcticClient.tick(Compat.screen() != null);
	}
}
