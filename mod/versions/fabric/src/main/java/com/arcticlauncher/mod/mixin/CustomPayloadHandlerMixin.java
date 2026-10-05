//#if MC >= 1.20.5
package com.arcticlauncher.mod.mixin;

//#if MC >= 1.20.5
import java.net.InetSocketAddress;
import java.net.SocketAddress;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.voice.VoiceLink;
import com.arcticlauncher.mod.svc.SvcPayload;
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The server's Simple Voice Chat secret goes to voice chat; a server that lists {@code arctic:hello}
 * gets Arctic's hello; its other messages are dropped quietly.
 */
@Mixin(ClientCommonPacketListenerImpl.class)
abstract class CustomPayloadHandlerMixin {
	@Shadow
	@Final
	protected Connection connection;

	/** The connection Arctic already said hello on, and to whom: once each is enough. */
	private static Connection arctic$greeted;
	private static final java.util.Set<String> arctic$greetings = new java.util.HashSet<>();

	@Inject(method = "handleCustomPayload(Lnet/minecraft/network/protocol/common/ClientboundCustomPayloadPacket;)V",
			at = @At("HEAD"), cancellable = true)
	private void arctic$svcSecret(ClientboundCustomPayloadPacket packet, CallbackInfo ci) {
		if (!(packet.payload() instanceof SvcPayload payload)) {
			return;
		}
		ci.cancel();
		arctic$greet(payload);
		VoiceLink voice = ArcticClient.voice();
		if (voice == null || !SvcPayload.SECRET.equals(payload.type().id())) {
			return;
		}
		SocketAddress remote = connection.getRemoteAddress();
		if (remote instanceof InetSocketAddress inet) {
			String host = inet.getAddress() != null ? inet.getAddress().getHostAddress() : inet.getHostString();
			voice.onSimpleVoiceChatSecret(payload.data(), host);
		}
	}

	/** Say hello to the servers that list a hello channel (Arctic's, and Polarium's on its behalf). */
	private void arctic$greet(SvcPayload register) {
		java.util.List<String> channels = register.channels();
		if (channels.isEmpty()) {
			return;
		}
		if (connection != arctic$greeted) {
			arctic$greeted = connection;
			arctic$greetings.clear();
		}
		if (channels.contains(SvcPayload.HELLO.toString()) && arctic$greetings.add("arctic")) {
			connection.send(new ServerboundCustomPayloadPacket(SvcPayload.hello()));
		}
		if (channels.contains(SvcPayload.POLARIUM_HELLO.toString()) && arctic$greetings.add("polarium")) {
			SvcPayload hello = SvcPayload.polariumHello();
			if (hello != null) {
				connection.send(new ServerboundCustomPayloadPacket(hello));
			}
		}
	}
}
//#endif
//#endif
