//#if MC >= 1.15
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Recorder;
import com.arcticlauncher.mod.replay.ReplayConnection;
import com.arcticlauncher.mod.replay.ReplayPlayback;
import com.arcticlauncher.mod.replay.ReplayRecording;
import io.netty.channel.ChannelHandlerContext;
import net.minecraft.network.Connection;
import net.minecraft.network.PacketListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Replays on the game's connection: each one that logs in records what it
 * receives (in the order it arrives), and a replay's own connection is
 * marked so it's never recorded.
 */
@Mixin(Connection.class)
abstract class ConnectionReplayMixin implements ReplayConnection {
	@Shadow
	@Final
	private PacketFlow receiving;

	/** What incoming packets are read with (see ReplayCodec): a ProtocolInfo, or before 1.20.5 the phase. */
	@Unique
	private volatile Object arctic$inbound;
	@Unique
	private volatile Recorder.Take arctic$take;
	@Unique
	private boolean arctic$replay;
	@Unique
	private boolean arctic$retired;

	//#if MC >= 1.20.5
	@Inject(method = "setupInboundProtocol", at = @At("HEAD"))
	private void arctic$protocol(net.minecraft.network.ProtocolInfo<?> protocol, PacketListener listener, CallbackInfo ci) {
		if (receiving != PacketFlow.CLIENTBOUND) {
			return;
		}
		arctic$inbound = protocol;
		if (!arctic$replay && arctic$take == null && protocol.id() == net.minecraft.network.ConnectionProtocol.LOGIN) {
			arctic$take = ReplayRecording.begin((Connection) (Object) this);
		}
	}
	//#else
	/** Before 1.20.5: a login listener starts a recording; the phase is followed packet by packet. */
	@Inject(method = "setListener", at = @At("HEAD"))
	private void arctic$listener(PacketListener listener, CallbackInfo ci) {
		if (receiving != PacketFlow.CLIENTBOUND || arctic$replay || arctic$take != null) {
			return;
		}
		if (listener instanceof net.minecraft.network.protocol.login.ClientLoginPacketListener) {
			arctic$inbound = com.arcticlauncher.mod.replay.ReplayCodec.loginProtocol();
			arctic$take = ReplayRecording.begin((Connection) (Object) this);
		}
	}
	//#endif

	@Inject(method = "channelRead0(Lio/netty/channel/ChannelHandlerContext;Lnet/minecraft/network/protocol/Packet;)V", at = @At("HEAD"))
	private void arctic$record(ChannelHandlerContext ctx, Packet<?> packet, CallbackInfo ci) {
		Recorder.Take take = arctic$take;
		Object protocol = arctic$inbound;
		if (take != null && protocol != null) {
			ReplayRecording.record(take, protocol, packet);
			//#if MC < 1.20.5
			arctic$inbound = com.arcticlauncher.mod.replay.ReplayCodec.next(protocol, packet);
			//#endif
		}
	}

	@Inject(method = "handleDisconnection", at = @At("HEAD"), cancellable = true)
	private void arctic$closed(CallbackInfo ci) {
		Recorder.Take take = arctic$take;
		if (take != null) {
			arctic$take = null;
			take.end();
		}
		if (arctic$retired) {
			// Replaced by a fresh replay connection: no "disconnected" screen.
			ci.cancel();
		} else if (arctic$replay) {
			ReplayPlayback.INSTANCE.closed((Connection) (Object) this);
		}
	}

	@Override
	public Object arctic$inbound() {
		return arctic$inbound;
	}

	@Override
	public void arctic$markReplay() {
		arctic$replay = true;
	}

	@Override
	public boolean arctic$isReplay() {
		return arctic$replay;
	}

	@Override
	public void arctic$retire() {
		arctic$retired = true;
	}

	@Override
	public boolean arctic$isRetired() {
		return arctic$retired;
	}
}
//#endif
