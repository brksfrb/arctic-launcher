package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.hud.Tps;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.protocol.game.ClientboundSetTimePacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The server's game time, for the TPS widget (first seen on arrival, before the main-thread hop). */
@Mixin(ClientPacketListener.class)
abstract class ServerTimeMixin {
	@Inject(method = "handleSetTime", at = @At("HEAD"))
	private void arctic$serverTime(ClientboundSetTimePacket packet, CallbackInfo ci) {
		//#if MC >= 1.21.2
		Tps.onServerTime(packet.gameTime());
		//#else
		Tps.onServerTime(packet.getGameTime());
		//#endif
	}
}
