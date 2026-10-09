package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.svc.SvcPayload;
import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.20.2
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
//#else
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundCustomPayloadPacket;
import net.minecraft.resources.Identifier;
//#endif

/**
 * The server's Simple Voice Chat secret goes to voice chat; a server that lists {@code arctic:hello}
 * gets Arctic's hello; its other messages are dropped quietly.
 */
//#if MC >= 1.20.2
@Mixin(ClientCommonPacketListenerImpl.class)
abstract class CustomPayloadHandlerMixin {
	@Shadow
	@Final
	protected Connection connection;

	@Inject(method = "handleCustomPayload(Lnet/minecraft/network/protocol/common/ClientboundCustomPayloadPacket;)V",
			at = @At("HEAD"), cancellable = true)
	private void arctic$svcSecret(ClientboundCustomPayloadPacket packet, CallbackInfo ci) {
		if (!(packet.payload() instanceof SvcPayload)) {
			return;
		}
		ci.cancel();
		SvcPayload payload = (SvcPayload) packet.payload();
		SvcPayload.received(connection, payload.channel(), payload.data());
	}
}
//#else
// Before 1.20.2 the packet is a channel plus a buffer, and the game logs every channel it doesn't know.
@Mixin(ClientPacketListener.class)
abstract class CustomPayloadHandlerMixin {
	@Shadow
	@Final
	private Connection connection;

	@Inject(method = "handleCustomPayload(Lnet/minecraft/network/protocol/game/ClientboundCustomPayloadPacket;)V",
			at = @At("HEAD"), cancellable = true)
	private void arctic$svcSecret(ClientboundCustomPayloadPacket packet, CallbackInfo ci) {
		Identifier id = packet.getIdentifier();
		// The packet comes by twice: on the network thread (the game then queues it) and on the game's own.
		// Take it on the second, and only the channels that are ours.
		if (!SvcPayload.handles(id) || !Minecraft.getInstance().isSameThread()) {
			return;
		}
		FriendlyByteBuf data = packet.getData();
		try {
			SvcPayload.received(connection, id, SvcPayload.read(data));
		} finally {
			// The game releases the buffer after it has handled the packet; we are handling it instead.
			data.release();
		}
		ci.cancel();
	}
}
//#endif
