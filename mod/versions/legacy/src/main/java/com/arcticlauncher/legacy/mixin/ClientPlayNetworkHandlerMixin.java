package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.hud.Tps;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.packet.s2c.play.WorldTimeUpdateS2CPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The server's game time, for the TPS widget. */
@Mixin(ClientPlayNetworkHandler.class)
abstract class ClientPlayNetworkHandlerMixin {
	@Inject(method = "onWorldTimeUpdate", at = @At("HEAD"))
	private void arctic$serverTime(WorldTimeUpdateS2CPacket packet, CallbackInfo ci) {
		Tps.onServerTime(packet.getTime());
	}
}
