package com.arcticlauncher.legacy.replay;

import com.arcticlauncher.client.replay.Recorder;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.SelfSample;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.Packet;
import net.minecraft.server.integrated.IntegratedServer;

/** Recording on old versions: every connection that logs in, plus where you are each tick. */
public final class LegacyReplayRecording {
	private static volatile boolean swung;

	private LegacyReplayRecording() {}

	/** A client connection is logging in: start a recording (null when off). */
	public static Recorder.Take begin(ClientConnection connection) {
		Recorder recorder = Replays.recorder();
		if (recorder == null) {
			return null;
		}
		MinecraftClient mc = MinecraftClient.getInstance();
		IntegratedServer local = mc.getServer();
		boolean singleplayer = local != null && connection.isLocal();
		String name;
		if (singleplayer) {
			name = local.getLevelName();
		} else {
			SocketAddress address = connection.getAddress();
			name = address instanceof InetSocketAddress ? ((InetSocketAddress) address).getHostString() : "Server";
		}
		return recorder.begin(name, singleplayer, LegacyReplayCodec.protocolVersion(), LegacyReplayView.VERSION);
	}

	/**
	 * Recording was asked for inside a world that was not being buffered (Replay was off when you
	 * joined, or was switched on since): start a buffer now, opening with the world as it is.
	 * Game thread only. False if this version can't, or you aren't in a world.
	 */
	public static boolean beginInWorld() {
		Recorder recorder = Replays.recorder();
		MinecraftClient mc = MinecraftClient.getInstance();
		if (recorder == null || mc.world == null || mc.player == null || mc.getNetworkHandler() == null || Replays.watching()) {
			return false;
		}
		if (recorder.current() != null) {
			return true;
		}
		if (!LegacyReplaySnapshot.supported()) {
			return false;
		}
		ClientConnection connection = mc.getNetworkHandler().getClientConnection();
		if (!(connection instanceof LegacyReplayConnection)) {
			return false;
		}
		Recorder.Take take = begin(connection);
		if (take == null) {
			return false;
		}
		((LegacyReplayConnection) connection).arctic$attach(take);
		//#if MC < 1.9
		int packets = LegacyReplaySnapshot.write(take);
		com.arcticlauncher.legacy.ArcticLegacy.LOG.info("replay: no buffer from the join; started one in the world with " + packets + " opening packets");
		//#endif
		return true;
	}

	/** A packet arrived (network thread). */
	public static void record(Recorder.Take take, Packet<?> packet) {
		byte[] bytes = LegacyReplayCodec.encode(packet);
		if (bytes != null) {
			take.packet(bytes);
		}
	}

	public static void swung() {
		swung = true;
	}

	/** Each client tick: pauses, the server's name, and where you are. */
	public static void tick() {
		Recorder recorder = Replays.recorder();
		Recorder.Take take = recorder == null ? null : recorder.current();
		if (take == null || Replays.watching()) {
			swung = false;
			return;
		}
		MinecraftClient mc = MinecraftClient.getInstance();
		take.paused(mc.isPaused());
		if (mc.getCurrentServerEntry() != null) {
			take.name(mc.getCurrentServerEntry().address);
		}
		ClientPlayerEntity p = mc.player;
		if (p == null || mc.world == null) {
			return;
		}
		take.self(p.getEntityId(), p.getGameProfile().getName(), p.getUuid());
		SelfSample s = new SelfSample();
		s.x = p.x;
		s.y = p.y;
		s.z = p.z;
		s.yaw = p.yaw;
		s.pitch = p.pitch;
		s.headYaw = p.headYaw;
		s.bodyYaw = p.bodyYaw;
		s.flags = (p.isSneaking() ? SelfSample.SNEAKING : 0)
				| (p.isSprinting() ? SelfSample.SPRINTING : 0)
				| (p.onGround ? SelfSample.ON_GROUND : 0);
		s.slot = p.inventory.selectedSlot;
		s.swing = swung ? 1 : 0;
		swung = false;
		s.health = p.getHealth();
		s.food = p.getHungerManager().getFoodLevel();
		s.fov = mc.options.fov;
		take.sample(s);
	}
}
