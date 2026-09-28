//#if MC >= 1.20.5
package com.arcticlauncher.mod.svc;

//#if MC >= 1.20.5
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;

/**
 * Simple Voice Chat's game-connection messages, kept as raw bytes: the
 * secret request, the server's secret (parsed by the launcher) and the
 * channel registration Paper needs before it sends the secret.
 */
public record SvcPayload(CustomPacketPayload.Type<SvcPayload> type, byte[] data) implements CustomPacketPayload {
	//#if MC >= 1.21
	public static final Identifier SECRET = Identifier.fromNamespaceAndPath("voicechat", "secret");
	public static final Identifier REQUEST = Identifier.fromNamespaceAndPath("voicechat", "request_secret");
	public static final Identifier REGISTER = Identifier.withDefaultNamespace("register");
	//#else
	// Before 1.21 there was no fromNamespaceAndPath/withDefaultNamespace, just constructors.
	public static final Identifier SECRET = new Identifier("voicechat", "secret");
	public static final Identifier REQUEST = new Identifier("voicechat", "request_secret");
	public static final Identifier REGISTER = new Identifier("register");
	//#endif
	/** Anything bigger isn't a secret. */
	private static final int MAX = 4096;
	/** SVC's compatibility version for 2.6.x. */
	private static final int COMPATIBILITY_VERSION = 20;

	/** The Simple Voice Chat mod handles its own messages when installed. */
	public static final boolean ENABLED = !FabricLoader.getInstance().isModLoaded("voicechat");
	/** Fabric API owns channel registration when it's installed. */
	private static final boolean OWN_REGISTER = !FabricLoader.getInstance().isModLoaded("fabric-networking-api-v1");

	/** Whether Arctic reads and writes this payload itself. */
	public static boolean handles(Identifier id) {
		return ENABLED && (SECRET.equals(id) || REQUEST.equals(id) || (OWN_REGISTER && REGISTER.equals(id)));
	}

	public static StreamCodec<FriendlyByteBuf, SvcPayload> codec(Identifier id) {
		final CustomPacketPayload.Type<SvcPayload> type = new CustomPacketPayload.Type<>(id);
		return StreamCodec.of((buf, payload) -> buf.writeBytes(payload.data()), buf -> {
			int n = buf.readableBytes();
			if (n > MAX) {
				buf.skipBytes(n);
				return new SvcPayload(type, new byte[0]);
			}
			byte[] data = new byte[n];
			buf.readBytes(data);
			return new SvcPayload(type, data);
		});
	}

	/** Register for the secret (for Paper), then ask for it. */
	public static SvcPayload[] request() {
		java.util.List<SvcPayload> out = new java.util.ArrayList<>();
		if (OWN_REGISTER) {
			byte[] channels = SECRET.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
			out.add(new SvcPayload(new CustomPacketPayload.Type<>(REGISTER), channels));
		}
		byte[] version = java.nio.ByteBuffer.allocate(4).putInt(COMPATIBILITY_VERSION).array();
		out.add(new SvcPayload(new CustomPacketPayload.Type<>(REQUEST), version));
		return out.toArray(new SvcPayload[0]);
	}
}
//#endif
//#endif
