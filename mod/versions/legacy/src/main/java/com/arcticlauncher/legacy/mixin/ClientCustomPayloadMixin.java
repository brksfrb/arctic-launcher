package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacySvc;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.packet.s2c.play.CustomPayloadS2CPacket;
import net.minecraft.util.PacketByteBuf;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The server's Simple Voice Chat secret goes to voice chat; a server that lists {@code arctic:hello}
 * gets Arctic's hello (its channel list arrives on {@code REGISTER}); these channels are taken quietly.
 */
@Mixin(ClientPlayNetworkHandler.class)
abstract class ClientCustomPayloadMixin {
	@Shadow
	@Final
	private ClientConnection connection;

	@Inject(method = "onCustomPayload", at = @At("HEAD"), cancellable = true)
	private void arctic$customPayload(CustomPayloadS2CPacket packet, CallbackInfo ci) {
		String channel = packet.getChannel();
		// The packet comes by twice: on the network thread (the game then queues it) and on the game's own.
		// Take it on the second, and only the channels that are ours.
		if (!LegacySvc.handles(channel) || !MinecraftClient.getInstance().isOnThread()) {
			return;
		}
		PacketByteBuf data = packet.getPayload();
		try {
			LegacySvc.received(connection, channel, LegacySvc.read(data));
		} finally {
			// The game releases the buffer after it has handled the packet; we are handling it instead.
			data.release();
		}
		ci.cancel();
	}
}
