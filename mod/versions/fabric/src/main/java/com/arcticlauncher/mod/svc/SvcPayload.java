package com.arcticlauncher.mod.svc;

import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.voice.VoiceLink;
import net.minecraft.network.Connection;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.resources.Identifier;
//#if MC >= 1.20.2
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
//#endif
//#if MC >= 1.20.5
import net.minecraft.network.codec.StreamCodec;
//#endif

/**
 * Simple Voice Chat's game-connection messages, kept as raw bytes: the
 * secret request, the server's secret (parsed by the launcher) and the
 * channel registration Paper needs before it sends the secret.
 *
 * <p>Custom payloads changed shape three times: a channel plus a buffer (before 1.20.2), a
 * {@code CustomPacketPayload} that the game reads with a per-id table (1.20.2-1.20.4), and one with a
 * codec (1.20.5+). The shared logic below is written once, in Java 8 (1.15 and 1.16 compile with it).
 */
//#if MC >= 1.20.5
public record SvcPayload(CustomPacketPayload.Type<SvcPayload> type, byte[] data) implements CustomPacketPayload {
	public Identifier channel() {
		return type.id();
	}

	private static SvcPayload of(Identifier id, byte[] data) {
		return new SvcPayload(new CustomPacketPayload.Type<>(id), data);
	}
//#elif MC >= 1.20.2
public final class SvcPayload implements CustomPacketPayload {
	private final Identifier channel;
	private final byte[] data;

	private SvcPayload(Identifier channel, byte[] data) {
		this.channel = channel;
		this.data = data;
	}

	public Identifier channel() {
		return channel;
	}

	public byte[] data() {
		return data;
	}

	private static SvcPayload of(Identifier id, byte[] data) {
		return new SvcPayload(id, data);
	}

	@Override
	public void write(FriendlyByteBuf buf) {
		buf.writeBytes(data);
	}

	@Override
	public Identifier id() {
		return channel;
	}
//#else
public final class SvcPayload {
	private final Identifier channel;
	private final byte[] data;

	private SvcPayload(Identifier channel, byte[] data) {
		this.channel = channel;
		this.data = data;
	}

	public Identifier channel() {
		return channel;
	}

	public byte[] data() {
		return data;
	}

	private static SvcPayload of(Identifier id, byte[] data) {
		return new SvcPayload(id, data);
	}
//#endif

	//#if MC >= 1.21
	public static final Identifier SECRET = Identifier.fromNamespaceAndPath("voicechat", "secret");
	public static final Identifier REQUEST = Identifier.fromNamespaceAndPath("voicechat", "request_secret");
	public static final Identifier REGISTER = Identifier.withDefaultNamespace("register");
	/** Tells a server that wants to know that Arctic is here (see {@link #hello()}). */
	public static final Identifier HELLO = Identifier.fromNamespaceAndPath("arctic", "hello");
	/** The same for Polarium, which relies on Arctic to say it when both are installed. */
	public static final Identifier POLARIUM_HELLO = Identifier.fromNamespaceAndPath("polarium", "hello");
	//#else
	// Before 1.21 there was no fromNamespaceAndPath/withDefaultNamespace, just constructors.
	public static final Identifier SECRET = new Identifier("voicechat", "secret");
	public static final Identifier REQUEST = new Identifier("voicechat", "request_secret");
	public static final Identifier REGISTER = new Identifier("register");
	public static final Identifier HELLO = new Identifier("arctic", "hello");
	public static final Identifier POLARIUM_HELLO = new Identifier("polarium", "hello");
	//#endif
	/** Anything bigger isn't a secret. */
	private static final int MAX = 4096;
	/** SVC's compatibility version for 2.6.x. */
	private static final int COMPATIBILITY_VERSION = 20;

	/** The Simple Voice Chat mod handles its own messages when installed. */
	public static final boolean ENABLED = !com.arcticlauncher.mod.Mods.loaded("voicechat");
	//#if NEOFORGE
	/** NeoForge reads the server's channel list itself (and lets a client send only on channels in it). */
	private static final boolean OWN_REGISTER = false;
	//#else
	/** Fabric API owns channel registration when it's installed. */
	private static final boolean OWN_REGISTER = !com.arcticlauncher.mod.Mods.loaded("fabric-networking-api-v1");
	//#endif

	/** Whether Arctic reads and writes this payload itself. */
	public static boolean handles(Identifier id) {
		// The hello and the server's channel list are Arctic's own, whether or not voice chat is.
		return HELLO.equals(id) || POLARIUM_HELLO.equals(id) || (OWN_REGISTER && REGISTER.equals(id)) || (ENABLED && (SECRET.equals(id) || REQUEST.equals(id)));
	}

	/** The channels a server announced in its {@code minecraft:register}, separated by NUL characters. */
	static List<String> channelsIn(byte[] data) {
		List<String> out = new ArrayList<>();
		for (String name : new String(data, StandardCharsets.UTF_8).split("\0")) {
			if (!name.isEmpty()) {
				out.add(name);
			}
		}
		return out;
	}

	/** What the server sent on a channel Arctic {@linkplain #handles handles}: its channel list or its voice chat secret. */
	public static void received(Connection connection, Identifier channel, byte[] data) {
		if (REGISTER.equals(channel)) {
			// Say hello to the servers that list a hello channel (Arctic's, and Polarium's on its behalf).
			Greeter.greet(connection, channelsIn(data));
			return;
		}
		VoiceLink voice = ArcticClient.voice();
		if (voice == null || !SECRET.equals(channel)) {
			return;
		}
		SocketAddress remote = connection.getRemoteAddress();
		if (remote instanceof InetSocketAddress) {
			InetSocketAddress inet = (InetSocketAddress) remote;
			String host = inet.getAddress() != null ? inet.getAddress().getHostAddress() : inet.getHostString();
			voice.onSimpleVoiceChatSecret(data, host);
		}
	}

	/** The payload bytes, consumed from the buffer; anything over the cap is skipped (and empty). */
	public static byte[] read(FriendlyByteBuf buf) {
		int n = buf.readableBytes();
		if (n > MAX) {
			buf.skipBytes(n);
			return new byte[0];
		}
		byte[] data = new byte[n];
		buf.readBytes(data);
		return data;
	}

	/** A payload for {@code id} from the bytes still in the buffer (what the game's own reader would drop). */
	public static SvcPayload readFrom(Identifier id, FriendlyByteBuf buf) {
		return of(id, read(buf));
	}

	/** The packet that sends this payload to the server. */
	public Packet<?> toPacket() {
		//#if MC >= 1.20.2
		return new net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(this);
		//#else
		return new net.minecraft.network.protocol.game.ServerboundCustomPayloadPacket(channel,
				new FriendlyByteBuf(io.netty.buffer.Unpooled.wrappedBuffer(data)));
		//#endif
	}

	/**
	 * The one message Arctic sends a server that asked for it (by listing
	 * {@code arctic:hello} among its channels): the mod's version and that the
	 * launcher is Arctic, as UTF-8 JSON. Nothing about the player or the PC.
	 */
	public static SvcPayload hello() {
		String version = com.arcticlauncher.mod.Mods.version("arctic").orElse("unknown");
		com.google.gson.JsonObject json = new com.google.gson.JsonObject();
		json.addProperty("version", version);
		json.addProperty("launcher", "arctic");
		return of(HELLO, json.toString().getBytes(StandardCharsets.UTF_8));
	}

	/**
	 * Polarium's hello ({@code {"version":"<its version>"}}), or null without Polarium. Polarium
	 * leaves it to Arctic when both are installed: only one of them can read the server's channel list.
	 */
	public static SvcPayload polariumHello() {
		java.util.Optional<String> polarium = com.arcticlauncher.mod.Mods.version("polarium");
		if (!polarium.isPresent()) {
			return null;
		}
		com.google.gson.JsonObject json = new com.google.gson.JsonObject();
		json.addProperty("version", polarium.get());
		return of(POLARIUM_HELLO, json.toString().getBytes(StandardCharsets.UTF_8));
	}

	//#if MC >= 1.20.5
	public static StreamCodec<FriendlyByteBuf, SvcPayload> codec(Identifier id) {
		final CustomPacketPayload.Type<SvcPayload> type = new CustomPacketPayload.Type<>(id);
		return StreamCodec.of((buf, payload) -> buf.writeBytes(payload.data()), buf -> new SvcPayload(type, read(buf)));
	}
	//#endif

	/** Register for the secret (for Paper), then ask for it: the packets to send. */
	public static List<Packet<?>> request() {
		List<Packet<?>> out = new ArrayList<>();
		//#if NEOFORGE && MC >= 1.20.2
		// NeoForge's own channel list payload (it reads minecraft:register itself).
		out.add(new net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(
				new net.neoforged.neoforge.network.payload.MinecraftRegisterPayload(java.util.Collections.singleton(SECRET))));
		//#elif NEOFORGE
		out.add(of(REGISTER, SECRET.toString().getBytes(StandardCharsets.UTF_8)).toPacket());
		//#else
		if (OWN_REGISTER) {
			byte[] channels = SECRET.toString().getBytes(StandardCharsets.UTF_8);
			out.add(of(REGISTER, channels).toPacket());
		}
		//#endif
		byte[] version = java.nio.ByteBuffer.allocate(4).putInt(COMPATIBILITY_VERSION).array();
		out.add(of(REQUEST, version).toPacket());
		return out;
	}
}
