package com.arcticlauncher.legacy.replay;

import com.arcticlauncher.client.replay.PacketSorter;
import com.google.common.collect.BiMap;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.network.NetworkSide;
import net.minecraft.network.NetworkState;
import net.minecraft.network.Packet;
import net.minecraft.util.PacketByteBuf;

/**
 * Packets to bytes and back on old versions, and their modern names. Each
 * packet class belongs to exactly one phase here, so recording needs no
 * phase tracking: the class says it.
 */
public final class LegacyReplayCodec {
	private LegacyReplayCodec() {}

	/** The network protocol number of this version. */
	public static int protocolVersion() {
		//#if MC >= 1.12
		return 340;
		//#elif MC >= 1.11
		return 316;
		//#elif MC >= 1.10
		return 210;
		//#elif MC >= 1.9
		return 110;
		//#else
		return 47;
		//#endif
	}

	public static PacketSorter sorter() {
		return new PacketSorter(names(), false, false, true);
	}

	/** Packet names by id: login, (no configuration), play. */
	static String[][] names() {
		return new String[][] {table(NetworkState.LOGIN), new String[0], table(NetworkState.PLAY)};
	}

	@SuppressWarnings({"rawtypes", "unchecked"})
	private static String[] table(NetworkState state) {
		Map<NetworkSide, BiMap<Integer, Class<?>>> classes = (Map) ((com.arcticlauncher.legacy.mixin.NetworkStateAccess) (Object) state).arctic$packetClasses();
		List<String> out = new ArrayList<String>();
		BiMap<Integer, Class<?>> ids = classes.get(NetworkSide.CLIENTBOUND);
		if (ids == null) {
			return new String[0];
		}
		for (Map.Entry<Integer, Class<?>> e : ids.entrySet()) {
			int id = e.getKey();
			while (out.size() <= id) {
				out.add(null);
			}
			out.set(id, LegacyPacketNames.of(e.getValue()));
		}
		return out.toArray(new String[0]);
	}

	/** The packet's modern name ("" if the replay doesn't care). */
	public static String name(Packet<?> packet) {
		return LegacyPacketNames.of(packet.getClass());
	}

	/** Encode with the packet's own phase; null if it can't be. */
	@SuppressWarnings({"rawtypes", "unchecked"})
	public static byte[] encode(Packet packet) {
		// Left out of replays anyway; and writing some (a plugin message) empties them for the game.
		if (PacketSorter.dropped(name(packet))) {
			return null;
		}
		NetworkState state = NetworkState.getPacketHandlerState(packet);
		if (state == null) {
			return null;
		}
		ByteBuf raw = Unpooled.buffer(256);
		try {
			Integer id = state.getRawId(NetworkSide.CLIENTBOUND, packet);
			if (id == null) {
				return null;
			}
			PacketByteBuf buf = new PacketByteBuf(raw);
			buf.writeVarInt(id);
			packet.write(buf);
			byte[] bytes = new byte[raw.readableBytes()];
			raw.readBytes(bytes);
			return bytes;
		} catch (Exception e) {
			return null;
		} finally {
			raw.release();
		}
	}

	/** Decode recorded bytes in {@code state}. */
	@SuppressWarnings("rawtypes")
	public static Packet decode(NetworkState state, ByteBuffer bytes) throws IOException {
		PacketByteBuf buf = new PacketByteBuf(Unpooled.wrappedBuffer(bytes));
		int id = buf.readVarInt();
		Packet packet;
		try {
			packet = state.createPacket(NetworkSide.CLIENTBOUND, id);
		} catch (IllegalAccessException | InstantiationException e) {
			throw new IOException("packet " + id + ": " + e);
		}
		if (packet == null) {
			throw new IOException("unknown packet " + id);
		}
		packet.read(buf);
		return packet;
	}
}
