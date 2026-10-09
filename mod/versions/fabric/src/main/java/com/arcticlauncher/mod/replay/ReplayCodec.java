//#if MC >= 1.15
package com.arcticlauncher.mod.replay;

import com.arcticlauncher.client.replay.PacketSorter;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
//#if MC >= 1.19.4
import net.minecraft.network.protocol.BundlePacket;
//#endif
import net.minecraft.network.protocol.Packet;
//#if MC >= 1.20.5
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.login.LoginProtocols;
//#else
import net.minecraft.network.ConnectionProtocol;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.PacketFlow;
//#endif

/**
 * Packets to bytes and back with the game's own codecs, and each packet's
 * protocol name (what {@link PacketSorter} sorts by).
 *
 * <p>A "protocol" here is what the game reads packets with: from 1.20.5 a
 * {@code ProtocolInfo} (the connection's own, kept by ConnectionReplayMixin);
 * before that the phase ({@code ConnectionProtocol}), which the replay code
 * follows itself packet by packet ({@link #next}).
 */
public final class ReplayCodec {
	private ReplayCodec() {}

	public static int protocolVersion() {
		//#if MC >= 1.16
		return SharedConstants.getProtocolVersion();
		//#else
		return SharedConstants.getCurrentVersion().getProtocolVersion();
		//#endif
	}

	public static PacketSorter sorter() {
		//#if MC >= 1.20.2
		return new PacketSorter(names(), true, true);
		//#else
		return new PacketSorter(names(), false, false);
		//#endif
	}

	/** The protocol a login starts with (before 1.20.5; later the connection's own). */
	public static Object loginProtocol() {
		//#if MC >= 1.20.5
		return null;
		//#else
		return ConnectionProtocol.LOGIN;
		//#endif
	}

	/** The protocol for the packet after {@code packet} (a login or configuration ends; before 1.20.5). */
	public static Object next(Object protocol, Packet<?> packet) {
		//#if MC >= 1.20.5
		return protocol;
		//#elif MC >= 1.20.2
		ConnectionProtocol next = packet.nextProtocol();
		return next != null ? next : protocol;
		//#else
		return packet instanceof net.minecraft.network.protocol.login.ClientboundGameProfilePacket ? ConnectionProtocol.PLAY : protocol;
		//#endif
	}

	/**
	 * A replay connection is about to handle {@code packet}: switch its
	 * reading protocol the way the network code would have (1.20.2–1.20.4;
	 * the listeners check it).
	 */
	public static void beforeHandle(io.netty.channel.Channel channel, Packet<?> packet) {
		//#if MC >= 1.20.2 && MC < 1.20.5
		net.minecraft.network.ProtocolSwapHandler.swapProtocolIfNeeded(
				channel.attr(net.minecraft.network.Connection.ATTRIBUTE_CLIENTBOUND_PROTOCOL), packet);
		//#endif
	}

	// ---- Names -------------------------------------------------------------------

	/** Packet names by id for each phase (login, configuration, play). */
	public static String[][] names() {
		//#if MC >= 1.21.5
		return new String[][] {
				names(LoginProtocols.CLIENTBOUND_TEMPLATE.details()),
				names(ConfigurationProtocols.CLIENTBOUND_TEMPLATE.details()),
				names(GameProtocols.CLIENTBOUND_TEMPLATE.details()),
		};
		//#elif MC >= 1.21
		return new String[][] {
				names(LoginProtocols.CLIENTBOUND_TEMPLATE),
				names(ConfigurationProtocols.CLIENTBOUND_TEMPLATE),
				names(GameProtocols.CLIENTBOUND_TEMPLATE),
		};
		//#elif MC >= 1.20.5
		// 1.20.5-1.20.6 can't list them: read the ids off the codecs.
		return new String[][] {
				fromCodec(LoginProtocols.CLIENTBOUND.codec()),
				fromCodec(ConfigurationProtocols.CLIENTBOUND.codec()),
				fromCodec(GameProtocols.CLIENTBOUND.bind(net.minecraft.network.RegistryFriendlyByteBuf.decorator(
						net.minecraft.core.RegistryAccess.EMPTY)).codec()),
		};
		//#elif MC >= 1.20.2
		return new String[][] {byClass(ConnectionProtocol.LOGIN), byClass(ConnectionProtocol.CONFIGURATION), byClass(ConnectionProtocol.PLAY)};
		//#else
		return new String[][] {byClass(ConnectionProtocol.LOGIN), new String[0], byClass(ConnectionProtocol.PLAY)};
		//#endif
	}

	//#if MC >= 1.21.5
	private static String[] names(ProtocolInfo.Details details) {
		final List<String> out = new ArrayList<String>();
		details.listPackets((type, id) -> put(out, id, type.id().getPath()));
		return out.toArray(new String[0]);
	}
	//#elif MC >= 1.21
	private static String[] names(ProtocolInfo.Unbound<?, ?> unbound) {
		final List<String> out = new ArrayList<String>();
		unbound.listPackets((type, id) -> put(out, id, type.id().getPath()));
		return out.toArray(new String[0]);
	}
	//#elif MC >= 1.20.5
	/** The codec's id → packet type list (a private field of every id-dispatching codec). */
	private static String[] fromCodec(Object codec) {
		List<String> out = new ArrayList<String>();
		try {
			for (java.lang.reflect.Field f : codec.getClass().getDeclaredFields()) {
				if (!List.class.isAssignableFrom(f.getType())) {
					continue;
				}
				f.setAccessible(true);
				List<?> entries = (List<?>) f.get(codec);
				for (int id = 0; id < entries.size(); id++) {
					Object entry = entries.get(id);
					for (java.lang.reflect.Field ef : entry.getClass().getDeclaredFields()) {
						ef.setAccessible(true);
						Object value = ef.get(entry);
						if (value instanceof net.minecraft.network.protocol.PacketType) {
							put(out, id, ((net.minecraft.network.protocol.PacketType<?>) value).id().getPath());
						}
					}
				}
			}
		} catch (ReflectiveOperationException | RuntimeException e) {
			com.arcticlauncher.mod.ArcticMod.LOG.warn("replay: packet ids unreadable ({}); jumps will be slower", e.toString());
		}
		return out.toArray(new String[0]);
	}
	//#elif MC >= 1.18
	private static String[] byClass(ConnectionProtocol protocol) {
		List<String> out = new ArrayList<String>();
		for (it.unimi.dsi.fastutil.ints.Int2ObjectMap.Entry<Class<? extends Packet<?>>> e
				: protocol.getPacketsByIds(PacketFlow.CLIENTBOUND).int2ObjectEntrySet()) {
			put(out, e.getIntKey(), PacketNames.of(e.getValue()));
		}
		return out.toArray(new String[0]);
	}
	//#else
	/**
	 * Before 1.18 the id table was private: found by type (the protocol's flow
	 * map, then its class-to-id map), since field names differ at runtime.
	 */
	private static String[] byClass(ConnectionProtocol protocol) {
		List<String> out = new ArrayList<String>();
		try {
			Object set = null;
			for (java.lang.reflect.Field f : ConnectionProtocol.class.getDeclaredFields()) {
				if (!java.lang.reflect.Modifier.isStatic(f.getModifiers()) && java.util.Map.class.isAssignableFrom(f.getType())) {
					f.setAccessible(true);
					set = ((java.util.Map<?, ?>) f.get(protocol)).get(PacketFlow.CLIENTBOUND);
				}
			}
			if (set != null) {
				for (java.lang.reflect.Field f : set.getClass().getDeclaredFields()) {
					if (it.unimi.dsi.fastutil.objects.Object2IntMap.class.isAssignableFrom(f.getType())) {
						f.setAccessible(true);
						for (it.unimi.dsi.fastutil.objects.Object2IntMap.Entry<?> e
								: ((it.unimi.dsi.fastutil.objects.Object2IntMap<?>) f.get(set)).object2IntEntrySet()) {
							put(out, e.getIntValue(), PacketNames.of((Class<?>) e.getKey()));
						}
					}
				}
			}
		} catch (ReflectiveOperationException | RuntimeException e) {
			com.arcticlauncher.mod.ArcticMod.LOG.warn("replay: packet ids unreadable ({}); jumps will be slower", e.toString());
		}
		return out.toArray(new String[0]);
	}
	//#endif

	private static void put(List<String> out, int id, String name) {
		while (out.size() <= id) {
			out.add(null);
		}
		out.set(id, name);
	}

	/** The packet's protocol name, like "level_chunk_with_light" ("" if unknown). */
	public static String name(Packet<?> packet) {
		//#if MC >= 1.20.5
		return packet.type().id().getPath();
		//#else
		return PacketNames.of(packet.getClass());
		//#endif
	}

	// ---- Bytes ---------------------------------------------------------------------

	/** Encode {@code packet} (or each packet of a bundle) with {@code protocol}; empty if it can't be. */
	@SuppressWarnings({"unchecked", "rawtypes"})
	public static List<byte[]> encode(Object protocol, Packet<?> packet) {
		List<byte[]> out = new ArrayList<byte[]>(1);
		//#if MC >= 1.19.4
		if (packet instanceof BundlePacket) {
			for (Object sub : ((BundlePacket) packet).subPackets()) {
				out.addAll(encode(protocol, (Packet<?>) sub));
			}
			return out;
		}
		//#endif
		ByteBuf buf = Unpooled.buffer(256);
		try {
			//#if MC >= 1.20.5
			((ProtocolInfo) protocol).codec().encode(buf, packet);
			//#else
			//#if MC >= 1.20.2
			int id = ((ConnectionProtocol) protocol).codec(PacketFlow.CLIENTBOUND).packetId(packet);
			//#else
			int id = ((ConnectionProtocol) protocol).getPacketId(PacketFlow.CLIENTBOUND, packet);
			//#endif
			if (id < 0) {
				return out;
			}
			FriendlyByteBuf friendly = new FriendlyByteBuf(buf);
			friendly.writeVarInt(id);
			packet.write(friendly);
			//#endif
			byte[] bytes = new byte[buf.readableBytes()];
			buf.readBytes(bytes);
			out.add(bytes);
		} catch (Exception e) {
			// A packet this side can't write (a mod's own): left out of the replay.
		} finally {
			buf.release();
		}
		return out;
	}

	/** Decode recorded bytes with {@code protocol}. */
	@SuppressWarnings("rawtypes")
	public static Packet<?> decode(Object protocol, ByteBuffer bytes) {
		ByteBuf buf = Unpooled.wrappedBuffer(bytes);
		//#if MC >= 1.20.5
		return (Packet<?>) ((ProtocolInfo) protocol).codec().decode(buf);
		//#else
		FriendlyByteBuf friendly = new FriendlyByteBuf(buf);
		int id = friendly.readVarInt();
		//#if MC >= 1.20.2
		return ((ConnectionProtocol) protocol).codec(PacketFlow.CLIENTBOUND).createPacket(id, friendly);
		//#elif MC >= 1.17
		return ((ConnectionProtocol) protocol).createPacket(PacketFlow.CLIENTBOUND, id, friendly);
		//#else
		// Before 1.17 packets were made empty, then read.
		try {
			Packet<?> packet = ((ConnectionProtocol) protocol).createPacket(PacketFlow.CLIENTBOUND, id);
			if (packet == null) {
				throw new IllegalArgumentException("unknown packet " + id);
			}
			packet.read(friendly);
			return packet;
		} catch (Exception e) {
			throw new IllegalStateException("packet " + id + ": " + e, e);
		}
		//#endif
		//#endif
	}
}
//#endif
