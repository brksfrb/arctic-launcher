package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.BadgeTexture;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Drops GPU badge textures on resource reload and when leaving a world, so VRAM stays bounded. */
@Mixin(Minecraft.class)
abstract class BadgeTextureLifecycleMixin {
	//#if MC >= 1.14
	@Inject(method = "reloadResourcePacks()Ljava/util/concurrent/CompletableFuture;", at = @At("HEAD"))
	private void arctic$badgeReload(CallbackInfoReturnable<?> ci) {
		BadgeTexture.releaseAll();
	}
	//#else
	@Inject(method = "reloadResources()V", at = @At("HEAD"))
	private void arctic$badgeReload(CallbackInfo ci) {
		BadgeTexture.releaseAll();
	}
	//#endif

	//#if MC >= 1.21.11
	@Inject(method = "disconnectFromWorld", at = @At("HEAD"))
	private void arctic$badgeLeave(CallbackInfo ci) {
		BadgeTexture.releaseAll();
	}
	//#elif MC >= 1.20.2
	@Inject(method = "disconnect()V", at = @At("HEAD"))
	private void arctic$badgeLeave(CallbackInfo ci) {
		BadgeTexture.releaseAll();
	}
	//#elif MC >= 1.14
	@Inject(method = "clearLevel()V", at = @At("HEAD"))
	private void arctic$badgeLeave(CallbackInfo ci) {
		BadgeTexture.releaseAll();
	}
	//#else
	// 1.13 mappings name this MultiPlayerLevel; common.gradle rewrites ClientLevel for MC < 1.15.
	@Inject(method = "connect(Lnet/minecraft/client/multiplayer/ClientLevel;)V", at = @At("HEAD"))
	private void arctic$badgeLeave13(net.minecraft.client.multiplayer.ClientLevel world, CallbackInfo ci) {
		if (world == null) {
			BadgeTexture.releaseAll();
		}
	}
	//#endif
}
