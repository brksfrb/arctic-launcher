//#if MC >= 1.15
package com.arcticlauncher.mod.replay;

import com.arcticlauncher.client.replay.Recorder;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.SelfSample;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.Packet;
import net.minecraft.server.MinecraftServer;

/**
 * The recording side on this version: every connection that logs in
 * records what it receives, and each tick notes where you are and what
 * you're doing (servers never send you your own movement).
 */
public final class ReplayRecording {
	private static volatile boolean swung;

	private ReplayRecording() {}

	/** A client connection is logging in: start a recording (null when off). */
	public static Recorder.Take begin(Connection connection) {
		Recorder recorder = Replays.recorder();
		if (recorder == null) {
			return null;
		}
		Minecraft mc = Minecraft.getInstance();
		MinecraftServer local = mc.getSingleplayerServer();
		boolean singleplayer = local != null && connection.isMemoryConnection();
		String name;
		if (singleplayer) {
			//#if MC >= 1.16
			name = local.getWorldData().getLevelName();
			//#else
			name = local.getLevelName();
			//#endif
		} else {
			SocketAddress address = connection.getRemoteAddress();
			name = address instanceof InetSocketAddress ? ((InetSocketAddress) address).getHostString() : "Server";
		}
		return recorder.begin(name, singleplayer, ReplayCodec.protocolVersion(), ReplayCompat.versionName());
	}

	/** A packet arrived (network thread): write it down. */
	public static void record(Recorder.Take take, Object protocol, Packet<?> packet) {
		// Left out of replays anyway; and writing some (a plugin message, on older
		// versions) empties them for the game.
		if (com.arcticlauncher.client.replay.PacketSorter.dropped(ReplayCodec.name(packet))) {
			return;
		}
		List<byte[]> encoded = ReplayCodec.encode(protocol, packet);
		for (byte[] bytes : encoded) {
			take.packet(bytes);
		}
	}

	/** You swung your arm (attacking or using something). */
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
		Minecraft mc = Minecraft.getInstance();
		take.paused(mc.isPaused());
		ClientPacketListener connection = mc.getConnection();
		if (connection != null) {
			//#if MC >= 1.19.3
			ServerData server = connection.getServerData();
			//#else
			ServerData server = mc.getCurrentServer();
			//#endif
			if (server != null && server.ip != null) {
				take.name(server.ip);
			}
		}
		LocalPlayer p = mc.player;
		if (p == null || mc.level == null) {
			return;
		}
		take.self(p.getId(), ReplayCompat.name(p.getGameProfile()), p.getUUID());
		SelfSample s = new SelfSample();
		s.x = p.getX();
		s.y = p.getY();
		s.z = p.getZ();
		s.yaw = com.arcticlauncher.mod.Compat.yRot(p);
		s.pitch = com.arcticlauncher.mod.Compat.xRot(p);
		s.headYaw = p.getYHeadRot();
		s.bodyYaw = p.yBodyRot;
		s.flags = (p.isShiftKeyDown() ? SelfSample.SNEAKING : 0)
				| (p.isSprinting() ? SelfSample.SPRINTING : 0)
				| (p.isVisuallySwimming() ? SelfSample.SWIMMING : 0)
				| (p.isFallFlying() ? SelfSample.FLYING_ELYTRA : 0)
				| (p.isUsingItem() ? SelfSample.USING_ITEM : 0)
				| (ReplayCompat.onGround(p) ? SelfSample.ON_GROUND : 0);
		s.slot = ReplayCompat.selectedSlot(com.arcticlauncher.mod.Compat.inventory(p));
		s.swing = swung ? 1 : 0;
		swung = false;
		s.health = p.getHealth();
		s.food = p.getFoodData().getFoodLevel();
		//#if MC >= 1.19
		s.fov = mc.options.fov().get();
		//#else
		s.fov = (int) mc.options.fov;
		//#endif
		take.sample(s);
	}
}
//#endif
