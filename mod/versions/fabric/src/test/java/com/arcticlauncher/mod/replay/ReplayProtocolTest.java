//#if MC >= 1.20.2
package com.arcticlauncher.mod.replay;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.replay.ReplayData;
import io.netty.channel.embedded.EmbeddedChannel;
import java.lang.reflect.Proxy;
import java.io.DataOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import com.mojang.authlib.GameProfile;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.network.Connection;
import net.minecraft.network.ConnectionProtocol;
import net.minecraft.network.PacketListener;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.configuration.ClientConfigurationPacketListener;
import net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket;
import net.minecraft.network.protocol.configuration.ClientboundUpdateEnabledFeaturesPacket;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.ClientboundSetChunkCacheRadiusPacket;
import net.minecraft.network.protocol.game.ClientboundStartConfigurationPacket;
import net.minecraft.network.protocol.login.ClientLoginPacketListener;
//#if MC >= 1.20.5
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.login.LoginProtocols;
//#endif
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Uses real Minecraft codecs and connection handovers without a window, server or renderer. */
class ReplayProtocolTest {
	@TempDir Path dir;
	@BeforeAll static void bootstrap() {
		SharedConstants.tryDetectVersion();
		net.minecraft.server.Bootstrap.bootStrap();
	}

	//#if MC >= 1.20.5
	private static ProtocolInfo<ClientGamePacketListener> playProtocol() {
		//#if MC >= 1.21
		return GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(net.minecraft.core.RegistryAccess.EMPTY));
		//#else
		return GameProtocols.CLIENTBOUND.bind(RegistryFriendlyByteBuf.decorator(net.minecraft.core.RegistryAccess.EMPTY));
		//#endif
	}
	//#endif

	private static final class Session implements AutoCloseable {
		final Connection connection = new Connection(PacketFlow.CLIENTBOUND);
		final EmbeddedChannel channel = ReplayTransport.open(connection);
		final List<String> handled = new ArrayList<>();
		Object inbound;
		Object protocol;

		Session() {
			ClientLoginPacketListener login = (ClientLoginPacketListener) listener(ConnectionProtocol.LOGIN, ClientLoginPacketListener.class);
			//#if MC >= 1.20.5
			connection.initiateServerboundPlayConnection("replay.arctic", 25565, LoginProtocols.SERVERBOUND, LoginProtocols.CLIENTBOUND, login, false);
			//#else
			connection.initiateServerboundPlayConnection("replay.arctic", 25565, login);
			//#endif
			protocol = inbound = ReplayCodec.loginProtocol();
		}

		PacketListener listener(ConnectionProtocol phase, Class<?> type) {
			return (PacketListener) Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[]{type}, (p, method, args) -> {
				if (method.getName().equals("flow")) return PacketFlow.CLIENTBOUND;
				if (method.getName().equals("protocol")) return phase;
				if (args != null && args.length == 1 && args[0] instanceof Packet) {
					String name = ReplayCodec.name((Packet<?>) args[0]);
					handled.add(name);
					if (name.equals("game_profile") || name.equals("login_finished")) {
						install(true);
						//#if MC >= 1.20.5
						connection.send(net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket.INSTANCE);
						//#else
						connection.send(new net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket());
						//#endif
						outbound(true);
					} else if (name.equals("start_configuration")) {
						install(true);
						//#if MC >= 1.20.5
						connection.send(net.minecraft.network.protocol.game.ServerboundConfigurationAcknowledgedPacket.INSTANCE);
						//#else
						connection.send(new net.minecraft.network.protocol.game.ServerboundConfigurationAcknowledgedPacket());
						//#endif
						outbound(true);
					} else if (name.equals("finish_configuration")) {
						install(false);
						//#if MC >= 1.20.5
						connection.send(net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket.INSTANCE);
						//#else
						connection.send(new net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket());
						//#endif
						outbound(false);
					}
				}
				return method.getReturnType() == boolean.class ? true : null;
			});
		}

		void outbound(boolean configuration) {
			//#if MC >= 1.20.5
			if (configuration) {
				connection.setupOutboundProtocol(ConfigurationProtocols.SERVERBOUND);
			} else {
				//#if MC >= 1.21.5
				connection.setupOutboundProtocol(GameProtocols.SERVERBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(net.minecraft.core.RegistryAccess.EMPTY), new GameProtocols.Context() {
					public boolean hasInfiniteMaterials() { return false; }
					//#if MC >= 26.3
					public boolean canUseCommandBlocks() { return false; }
					//#endif
				}));
				//#elif MC >= 1.21
				connection.setupOutboundProtocol(GameProtocols.SERVERBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(net.minecraft.core.RegistryAccess.EMPTY)));
				//#else
				connection.setupOutboundProtocol(GameProtocols.SERVERBOUND.bind(RegistryFriendlyByteBuf.decorator(net.minecraft.core.RegistryAccess.EMPTY)));
				//#endif
			}
			assertNotNull(channel.pipeline().get("encoder"));
			//#endif
		}

		void install(boolean configuration) {
			ConnectionProtocol phase = configuration ? ConnectionProtocol.CONFIGURATION : ConnectionProtocol.PLAY;
			PacketListener listener = listener(phase, configuration ? ClientConfigurationPacketListener.class : ClientGamePacketListener.class);
			//#if MC >= 1.20.5
			if (configuration) {
				inbound = ConfigurationProtocols.CLIENTBOUND;
				connection.setupInboundProtocol(ConfigurationProtocols.CLIENTBOUND, (ClientConfigurationPacketListener) listener);
			} else {
				inbound = playProtocol();
				connection.setupInboundProtocol(playProtocol(), (ClientGamePacketListener) listener);
			}
			// Configuration tasks must execute, not accumulate among discarded outgoing packets.
			assertNotNull(channel.pipeline().get("decoder"));
			assertFalse(channel.outboundMessages().stream().anyMatch(m -> m instanceof net.minecraft.network.UnconfiguredPipelineHandler.InboundConfigurationTask));
			//#else
			inbound = phase;
			connection.setListener(listener);
			//#endif
		}

		@SuppressWarnings({"rawtypes", "unchecked"})
		void apply(byte[] bytes) {
			Packet<?> packet = ReplayCodec.decode(protocol, ByteBuffer.wrap(bytes));
			ReplayCodec.beforeHandle(channel, packet);
			((Packet) packet).handle(connection.getPacketListener());
			protocol = ReplayCodec.afterHandle(protocol, packet, inbound);
			channel.checkException();
			channel.releaseOutbound();
		}

		public void close() { channel.finishAndReleaseAll(); }
	}

	@Test void recordedReconfigurationCanReturnToPlayRepeatedly() throws Exception {
		//#if MC >= 1.20.5
		Object play = playProtocol();
		Object config = ConfigurationProtocols.CLIENTBOUND;
		Packet<?> start = ClientboundStartConfigurationPacket.INSTANCE;
		Packet<?> finish = ClientboundFinishConfigurationPacket.INSTANCE;
		//#else
		Object play = ConnectionProtocol.PLAY;
		Object config = ConnectionProtocol.CONFIGURATION;
		Packet<?> start = new ClientboundStartConfigurationPacket();
		Packet<?> finish = new ClientboundFinishConfigurationPacket();
		//#endif
		Packet<?>[] packets = {start, new ClientboundUpdateEnabledFeaturesPacket(Collections.emptySet()), finish,
				new ClientboundSetChunkCacheRadiusPacket(12)};
		List<byte[]> configPackets = new ArrayList<>();
		//#if MC >= 1.20.5
		configPackets.addAll(ReplayCodec.encode(config, new net.minecraft.network.protocol.configuration.ClientboundSelectKnownPacks(Collections.emptyList())));
		configPackets.addAll(ReplayCodec.encode(config, new net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket(net.minecraft.core.registries.Registries.DIMENSION_TYPE, Collections.emptyList())));
		assertEquals(2, configPackets.size());
		//#else
		configPackets.addAll(ReplayCodec.encode(config, new net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket(net.minecraft.core.RegistryAccess.EMPTY)));
		assertEquals(1, configPackets.size());
		//#endif
		List<byte[]> cycle = new ArrayList<>();
		for (int i = 0; i < packets.length; i++) {
			List<byte[]> encoded = ReplayCodec.encode(i == 1 || i == 2 ? config : play, packets[i]);
			assertEquals(1, encoded.size(), "Every phase-boundary packet must be recorded");
			cycle.add(encoded.get(0));
			if (i == 0) cycle.addAll(configPackets);
		}
		GameProfile profile = new GameProfile(new UUID(0, 1), "Replay");
		//#if MC >= 26.2
		Packet<?> login = new net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket(profile, new UUID(0, 2));
		//#elif MC >= 1.21.2
		Packet<?> login = new net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket(profile);
		//#elif MC >= 1.20.5
		Packet<?> login = new net.minecraft.network.protocol.login.ClientboundGameProfilePacket(profile, true);
		//#else
		Packet<?> login = new net.minecraft.network.protocol.login.ClientboundGameProfilePacket(profile);
		//#endif
		List<byte[]> recording = new ArrayList<>(ReplayCodec.encode(ReplayCodec.loginProtocol(), login));
		assertEquals(1, recording.size());
		// Initial configuration, then three proxy/server switches, with real outgoing acknowledgements.
		recording.addAll(configPackets);
		recording.addAll(ReplayCodec.encode(config, packets[1]));
		recording.addAll(ReplayCodec.encode(config, finish));
		for (int i = 0; i < 3; i++) recording.addAll(cycle);
		Path file = dir.resolve("old-format.mcpr");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
			zip.putNextEntry(new ZipEntry("metaData.json"));
			zip.write("{}".getBytes(java.nio.charset.StandardCharsets.UTF_8)); zip.closeEntry();
			// No phase metadata: the existing recording format must remain readable.
			zip.putNextEntry(new ZipEntry("recording.tmcpr"));
			DataOutputStream out = new DataOutputStream(zip);
			for (byte[] bytes : recording) { out.writeInt(0); out.writeInt(bytes.length); out.write(bytes); }
			zip.closeEntry();
		}
		try (ReplayData data = ReplayData.open(file.toFile(), dir.resolve("cache").toFile()); Session playback = new Session()) {
			assertEquals(recording.size(), data.count());
			for (int i = 0; i < data.count(); i++) {
				ByteBuffer bytes = data.packet(i);
				byte[] recorded = new byte[bytes.remaining()]; bytes.get(recorded);
				playback.apply(recorded);
			}
			assertTrue(ReplayCodec.context(playback.protocol, ByteBuffer.wrap(cycle.get(cycle.size() - 1))).contains("PLAY"));
			assertEquals(recording.size(), playback.handled.size());
			assertEquals("set_chunk_cache_radius", playback.handled.get(playback.handled.size() - 1));
		}

	}

	@Test void anUnencodableBoundaryEndsRecordingRatherThanOmittingThePhaseChange() {
		com.arcticlauncher.client.replay.Recorder recorder = new com.arcticlauncher.client.replay.Recorder(
				dir.toFile(), () -> 30, s -> {}, () -> null);
		com.arcticlauncher.client.replay.Recorder.Take take = recorder.begin("test", false, ReplayCodec.protocolVersion(), "test");
		//#if MC >= 1.20.5
		Object wrongProtocol = ConfigurationProtocols.CLIENTBOUND;
		Packet<?> start = ClientboundStartConfigurationPacket.INSTANCE;
		//#else
		Object wrongProtocol = ConnectionProtocol.CONFIGURATION;
		Packet<?> start = new ClientboundStartConfigurationPacket();
		//#endif
		ReplayRecording.record(take, wrongProtocol, start);
		assertFalse(recorder.bufferReady());
		assertNull(recorder.current());
	}

	//#if MC >= 1.20.5
	@Test void aDecodeFailurePreservesThePhaseAndWireIdForTheFirstErrorLog() {
		ByteBuffer bytes = ByteBuffer.wrap(new byte[]{51});
		assertThrows(RuntimeException.class, () -> ReplayCodec.decode(LoginProtocols.CLIENTBOUND, bytes));
		assertEquals(0, bytes.position());
		String context = ReplayCodec.context(LoginProtocols.CLIENTBOUND, bytes);
		assertTrue(context.contains("protocol=" + ReplayCodec.protocolVersion()));
		assertTrue(context.contains("phase=LOGIN"));
		assertTrue(context.contains("packetId=51"));
	}

	@Test void incompleteHandoverIsReportedBeforeReadingPlayBytesAsConfiguration() {
		assertThrows(IllegalStateException.class, () -> ReplayCodec.afterHandle(
				ConfigurationProtocols.CLIENTBOUND, ClientboundFinishConfigurationPacket.INSTANCE, ConfigurationProtocols.CLIENTBOUND));
	}
	//#endif
}
//#endif
