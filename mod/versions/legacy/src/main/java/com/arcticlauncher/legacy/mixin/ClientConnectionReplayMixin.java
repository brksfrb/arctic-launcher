package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Recorder;
import com.arcticlauncher.legacy.replay.LegacyReplayConnection;
import com.arcticlauncher.legacy.replay.LegacyReplayPlayback;
import com.arcticlauncher.legacy.replay.LegacyReplayRecording;
import io.netty.channel.ChannelHandlerContext;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.NetworkSide;
import net.minecraft.network.Packet;
import net.minecraft.network.listener.ClientLoginPacketListener;
import net.minecraft.network.listener.PacketListener;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Replays on the connection: each login records what it receives; a replay's own connection never does. */
@Mixin(ClientConnection.class)
abstract class ClientConnectionReplayMixin implements LegacyReplayConnection {
	@Shadow
	@Final
	private NetworkSide side;

	@Unique
	private volatile Recorder.Take arctic$take;
	@Unique
	private boolean arctic$replay;
	@Unique
	private boolean arctic$retired;

	@Inject(method = "setPacketListener", at = @At("HEAD"))
	private void arctic$listener(PacketListener listener, CallbackInfo ci) {
		if (side != NetworkSide.CLIENTBOUND || arctic$replay || arctic$take != null) {
			return;
		}
		if (listener instanceof ClientLoginPacketListener) {
			arctic$take = LegacyReplayRecording.begin((ClientConnection) (Object) this);
		}
	}

	@Inject(method = "channelRead0(Lio/netty/channel/ChannelHandlerContext;Lnet/minecraft/network/Packet;)V", at = @At("HEAD"))
	private void arctic$record(ChannelHandlerContext ctx, Packet<?> packet, CallbackInfo ci) {
		Recorder.Take take = arctic$take;
		if (take != null) {
			LegacyReplayRecording.record(take, packet);
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
			ci.cancel();
		} else if (arctic$replay) {
			LegacyReplayPlayback.INSTANCE.closed((ClientConnection) (Object) this);
		}
	}

	@Override
	public void arctic$markReplay() {
		arctic$replay = true;
	}

	@Override
	public void arctic$retire() {
		arctic$retired = true;
	}
}
