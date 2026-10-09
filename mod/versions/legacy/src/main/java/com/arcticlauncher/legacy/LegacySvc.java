package com.arcticlauncher.legacy;

import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.ByteBuffer;
import java.nio.charset.Charset;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.voice.VoiceLink;
import com.google.gson.JsonObject;
import io.netty.buffer.Unpooled;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.ModContainer;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.Packet;
import net.minecraft.network.packet.c2s.play.CustomPayloadC2SPacket;
import net.minecraft.util.PacketByteBuf;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Plugin-channel messages on 1.8-1.12, where a channel is a plain string and the server's channel
 * list comes on {@code REGISTER}: Simple Voice Chat's secret request and answer, and the hello that
 * Arctic ({@code arctic:hello}) and Polarium ({@code polarium:hello}) give a server that lists them,
 * once per connection. Same behaviour as the modern adapter's {@code SvcPayload} and {@code Greeter}.
 */
public final class LegacySvc {
	private static final Logger LOG = LogManager.getLogger("Arctic");
	private static final Charset UTF_8 = Charset.forName("UTF-8");

	/** Channels were registered with {@code REGISTER} (no namespace) until 1.13. */
	static final String REGISTER = "REGISTER";
	static final String SECRET = "voicechat:secret";
	static final String REQUEST = "voicechat:request_secret";
	/** Tells a server that wants to know that Arctic is here (see {@link #hello()}). */
	static final String HELLO = "arctic:hello";
	/** The same for Polarium, which relies on Arctic to say it when both are installed. */
	static final String POLARIUM_HELLO = "polarium:hello";

	/** Anything bigger isn't a secret. */
	private static final int MAX = 4096;
	/** A channel name is at most 20 characters up to 1.12: a longer one gets the client kicked. */
	private static final int MAX_CHANNEL = 20;
	/** SVC's compatibility version for 2.6.x. */
	private static final int COMPATIBILITY_VERSION = 20;

	/** The Simple Voice Chat mod handles its own messages when installed. */
	static final boolean ENABLED = !FabricLoader.getInstance().isModLoaded("voicechat");

	private static ClientConnection greeted;
	private static final Set<String> GREETINGS = new HashSet<String>();

	private LegacySvc() {}

	/** Whether Arctic reads this channel itself. */
	public static boolean handles(String channel) {
		// The hello and the server's channel list are Arctic's own, whether or not voice chat is.
		return REGISTER.equals(channel) || HELLO.equals(channel) || POLARIUM_HELLO.equals(channel)
				|| (ENABLED && (SECRET.equals(channel) || REQUEST.equals(channel)));
	}

	/** The payload bytes, consumed from the buffer; anything over the cap is skipped (and empty). */
	public static byte[] read(PacketByteBuf buf) {
		int n = buf.readableBytes();
		if (n > MAX) {
			buf.skipBytes(n);
			return new byte[0];
		}
		byte[] data = new byte[n];
		buf.readBytes(data);
		return data;
	}

	/** What the server sent on a channel Arctic {@linkplain #handles handles}: its channel list or its voice chat secret. */
	public static void received(ClientConnection connection, String channel, byte[] data) {
		if (REGISTER.equals(channel)) {
			greet(connection, channelsIn(data));
			return;
		}
		VoiceLink voice = ArcticClient.voice();
		if (voice == null || !SECRET.equals(channel) || connection == null) {
			return;
		}
		SocketAddress remote = connection.getAddress();
		if (remote instanceof InetSocketAddress) {
			InetSocketAddress inet = (InetSocketAddress) remote;
			String host = inet.getAddress() != null ? inet.getAddress().getHostAddress() : inet.getHostString();
			voice.onSimpleVoiceChatSecret(data, host);
		}
	}

	/** The channels a server announced in its {@code REGISTER}, separated by NUL characters. */
	static List<String> channelsIn(byte[] data) {
		List<String> out = new ArrayList<String>();
		for (String name : new String(data, UTF_8).split("\0")) {
			if (!name.isEmpty()) {
				out.add(name);
			}
		}
		return out;
	}

	/** The server on {@code connection} announced these channels. */
	private static synchronized void greet(ClientConnection connection, List<String> channels) {
		if (connection == null || channels.isEmpty()) {
			return;
		}
		boolean arctic = channels.contains(HELLO);
		boolean polarium = channels.contains(POLARIUM_HELLO);
		if (arctic || polarium) {
			LOG.info("Arctic: the server lists {} channel(s) (arctic:hello {}, polarium:hello {})", channels.size(), arctic, polarium);
		}
		if (connection != greeted) {
			greeted = connection;
			GREETINGS.clear();
		}
		if (arctic && GREETINGS.add("arctic")) {
			connection.send(packet(HELLO, hello()));
			LOG.info("Arctic: said arctic:hello");
		}
		if (polarium && GREETINGS.add("polarium")) {
			byte[] hello = polariumHello();
			if (hello != null) {
				connection.send(packet(POLARIUM_HELLO, hello));
				LOG.info("Arctic: said polarium:hello for Polarium");
			} else {
				LOG.info("Arctic: the server asks for polarium:hello but Polarium isn't loaded");
			}
		}
	}

	private static Packet<?> packet(String channel, byte[] data) {
		return new CustomPayloadC2SPacket(channel, new PacketByteBuf(Unpooled.wrappedBuffer(data)));
	}

	/**
	 * The one message Arctic sends a server that asked for it: the mod's version and that the
	 * launcher is Arctic, as UTF-8 JSON. Nothing about the player or the PC.
	 */
	private static byte[] hello() {
		ModContainer arctic = FabricLoader.getInstance().getModContainer("arctic").orElse(null);
		JsonObject json = new JsonObject();
		json.addProperty("version", arctic == null ? "unknown" : arctic.getMetadata().getVersion().getFriendlyString());
		json.addProperty("launcher", "arctic");
		return json.toString().getBytes(UTF_8);
	}

	/** Polarium's hello ({@code {"version":"<its version>"}}), or null without Polarium. */
	private static byte[] polariumHello() {
		ModContainer polarium = FabricLoader.getInstance().getModContainer("polarium").orElse(null);
		if (polarium == null) {
			return null;
		}
		JsonObject json = new JsonObject();
		json.addProperty("version", polarium.getMetadata().getVersion().getFriendlyString());
		return json.toString().getBytes(UTF_8);
	}

	/** Register for the secret (for Paper), then ask for it. */
	public static void request(ClientPlayNetworkHandler handler) {
		ClientConnection connection = handler.getClientConnection();
		send(connection, REGISTER, SECRET.getBytes(UTF_8));
		send(connection, REQUEST, ByteBuffer.allocate(4).putInt(COMPATIBILITY_VERSION).array());
	}

	private static void send(ClientConnection connection, String channel, byte[] data) {
		if (channel.length() > MAX_CHANNEL) {
			// The protocol can't carry it (the server would reject the packet): nothing to send.
			LOG.debug("Arctic: no room for channel {} on this version", channel);
			return;
		}
		connection.send(packet(channel, data));
	}
}
