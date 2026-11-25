package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.ArcticLegacy;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Drops GPU badge textures on resource reload and when leaving a world, so VRAM stays bounded. */
@Mixin(MinecraftClient.class)
abstract class BadgeTextureLifecycleMixin {
	@Inject(method = "reloadResources()V", at = @At("HEAD"), require = 0)
	private void arctic$badgeReload(CallbackInfo ci) {
		ArcticLegacy.releaseBadgeTextures();
	}

	@Inject(method = "connect(Lnet/minecraft/client/world/ClientWorld;Ljava/lang/String;)V", at = @At("HEAD"), require = 0)
	private void arctic$badgeLeave(ClientWorld world, String message, CallbackInfo ci) {
		if (world == null) {
			ArcticLegacy.releaseBadgeTextures();
		}
	}
}
