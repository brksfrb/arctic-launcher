package com.arcticlauncher.mod.mixin;

import net.minecraft.client.Minecraft;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.server.MinecraftServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A world opened to LAN (Play Together included) checks every joiner's Microsoft sign-in, so a friend
 * with an offline account was refused with "Invalid session". When the host plays offline themself
 * (the launcher gives offline accounts the token "0"), the world doesn't ask for the sign-in.
 */
@Mixin(MinecraftServer.class)
abstract class OfflineLanMixin {
	@Inject(method = "usesAuthentication", at = @At("RETURN"), cancellable = true)
	private void arctic$offlineHost(CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValueZ() && (Object) this instanceof IntegratedServer
				&& "0".equals(Minecraft.getInstance().getUser().getAccessToken())) {
			cir.setReturnValue(false);
		}
	}
}
