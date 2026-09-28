//#if MC >= 1.16
package com.arcticlauncher.mod.replay;

import com.mojang.authlib.GameProfile;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.network.Connection;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientboundLoginPacket;
//#if MC >= 1.19
import net.minecraft.network.protocol.game.ClientboundPlayerChatPacket;
//#endif
import net.minecraft.network.protocol.game.ClientboundRespawnPacket;
//#if MC >= 1.20.2
import net.minecraft.network.protocol.game.CommonPlayerSpawnInfo;
//#endif
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.level.GameType;

/** The replay code's version differences, in one place. */
public final class ReplayCompat {
	private ReplayCompat() {}

	public static void swing(LivingEntity entity) {
		//#if MC >= 26.3
		entity.swing(InteractionHand.MAIN_HAND, net.minecraft.world.item.component.SwingAnimation.DEFAULT, false);
		//#else
		entity.swing(InteractionHand.MAIN_HAND);
		//#endif
	}

	/** The world an entity is in. */
	static net.minecraft.world.level.Level level(Entity e) {
		//#if MC >= 1.20
		return e.level();
		//#else
		return e.level;
		//#endif
	}

	static boolean onGround(Entity e) {
		//#if MC >= 1.20
		return e.onGround();
		//#else
		return e.isOnGround();
		//#endif
	}

	/** "26.3", "1.21.4"… */
	static String versionName() {
		//#if MC >= 1.21.6
		return net.minecraft.SharedConstants.getCurrentVersion().name();
		//#else
		return net.minecraft.SharedConstants.getCurrentVersion().getName();
		//#endif
	}

	static int selectedSlot(Inventory inventory) {
		//#if MC >= 1.21.5
		return inventory.getSelectedSlot();
		//#else
		return inventory.selected;
		//#endif
	}

	static void selectSlot(Inventory inventory, int slot) {
		//#if MC >= 1.21.5
		inventory.setSelectedSlot(slot);
		//#else
		inventory.selected = slot;
		//#endif
	}

	/** Put an entity somewhere at once. */
	static void snap(Entity e, double x, double y, double z, float yaw, float pitch) {
		//#if MC >= 1.21.5
		e.snapTo(x, y, z, yaw, pitch);
		//#else
		e.moveTo(x, y, z, yaw, pitch);
		//#endif
	}

	/** Move an entity smoothly there over the next few ticks, as a server update would. */
	static void glide(Entity e, double x, double y, double z, float yaw, float pitch) {
		//#if MC >= 1.21.5
		e.moveOrInterpolateTo(new net.minecraft.world.phys.Vec3(x, y, z), yaw, pitch);
		//#elif MC >= 1.20.2
		e.lerpTo(x, y, z, yaw, pitch, 3);
		//#else
		e.lerpTo(x, y, z, yaw, pitch, 3, false);
		//#endif
	}

	/** Add a client-side entity to the world. */
	static void addEntity(net.minecraft.client.multiplayer.ClientLevel level, net.minecraft.client.player.AbstractClientPlayer player) {
		//#if MC >= 1.20.2
		level.addEntity(player);
		//#else
		level.addPlayer(player.getId(), player);
		//#endif
	}

	static String name(GameProfile profile) {
		//#if MC >= 1.21.9
		return profile.name();
		//#else
		return profile.getName();
		//#endif
	}

	static void systemChat(Component message) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		mc.gui.chatListener().handleSystemMessage(message, false);
		//#elif MC >= 1.19
		mc.getChatListener().handleSystemMessage(message, false);
		//#else
		mc.gui.handleChat(net.minecraft.network.chat.ChatType.SYSTEM, message, net.minecraft.Util.NIL_UUID);
		//#endif
	}

	//#if MC >= 1.19
	/** A signed chat message as it looked (its checks can't be redone later). */
	static Component chatText(ClientboundPlayerChatPacket p) {
		//#if MC >= 26.3
		Component content = p.unsignedContent().orElse(com.arcticlauncher.mod.Compat.literal(p.body().content()));
		//#elif MC >= 1.19.3
		Component content = p.unsignedContent() != null ? p.unsignedContent() : com.arcticlauncher.mod.Compat.literal(p.body().content());
		//#else
		Component content = p.message().unsignedContent().orElse(p.message().serverContent());
		//#endif
		//#if MC >= 1.20.5
		return p.chatType().decorate(content);
		//#else
		net.minecraft.client.multiplayer.ClientPacketListener connection = Minecraft.getInstance().getConnection();
		if (connection == null) {
			return content;
		}
		final Component body = content;
		//#if MC >= 1.19.3
		return p.chatType().resolve(connection.registryAccess()).map(bound -> bound.decorate(body)).orElse(body);
		//#else
		return p.resolveChatType(connection.registryAccess()).map(bound -> bound.decorate(body)).orElse(body);
		//#endif
		//#endif
	}

	//#endif

	static boolean hudHidden() {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		return mc.gui.hud.isHidden();
		//#else
		return mc.options.hideGui;
		//#endif
	}

	static void setHudHidden(boolean hidden) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		if (mc.gui.hud.isHidden() != hidden) {
			mc.gui.hud.toggle();
		}
		//#else
		mc.options.hideGui = hidden;
		//#endif
	}

	static com.mojang.blaze3d.pipeline.RenderTarget mainTarget() {
		//#if MC >= 26.2
		return Minecraft.getInstance().gameRenderer.mainRenderTarget();
		//#else
		return Minecraft.getInstance().getMainRenderTarget();
		//#endif
	}

	static void setScreen(net.minecraft.client.gui.screens.Screen screen) {
		com.arcticlauncher.mod.Compat.setScreen(screen);
	}

	/** Leave the world or replay, showing {@code screen}. */
	static void disconnect(net.minecraft.client.gui.screens.Screen screen) {
		net.minecraft.client.multiplayer.ClientPacketListener listener = Minecraft.getInstance().getConnection();
		if (listener != null && !((ReplayConnection) listener.getConnection()).arctic$isReplay()) {
			// Leaving a real world (not a replay's own connection).
			com.arcticlauncher.mod.Compat.quitLevel();
		}
		//#if MC >= 1.20.5
		Minecraft.getInstance().disconnect(screen, false);
		//#elif MC >= 1.20.2
		Minecraft.getInstance().disconnect(screen);
		//#else
		Minecraft.getInstance().clearLevel(screen);
		//#endif
	}

	/** A plain screen with one line ("Jumping…"). */
	static net.minecraft.client.gui.screens.Screen message(String text) {
		//#if MC >= 1.20.5
		return new net.minecraft.client.gui.screens.GenericMessageScreen(com.arcticlauncher.mod.Compat.literal(text));
		//#else
		return new net.minecraft.client.gui.screens.GenericDirtMessageScreen(com.arcticlauncher.mod.Compat.literal(text));
		//#endif
	}

	/** One of those message screens is showing. */
	public static boolean messageShowing() {
		net.minecraft.client.gui.screens.Screen s = com.arcticlauncher.mod.Compat.screen();
		//#if MC >= 1.20.5
		return s instanceof net.minecraft.client.gui.screens.GenericMessageScreen;
		//#else
		return s instanceof net.minecraft.client.gui.screens.GenericDirtMessageScreen;
		//#endif
	}

	/** The login listener a replay's connection starts with. */
	static ClientHandshakePacketListenerImpl loginListener(Connection conn) {
		Minecraft mc = Minecraft.getInstance();
		net.minecraft.client.gui.screens.Screen parent = new net.minecraft.client.gui.screens.TitleScreen();
		//#if MC >= 1.21.9
		return new ClientHandshakePacketListenerImpl(conn, mc, null, parent, false, null, status -> {},
				new net.minecraft.client.multiplayer.LevelLoadTracker(), null);
		//#elif MC >= 1.20.5
		return new ClientHandshakePacketListenerImpl(conn, mc, null, parent, false, null, status -> {}, null);
		//#elif MC >= 1.19.3
		return new ClientHandshakePacketListenerImpl(conn, mc, null, parent, false, null, status -> {});
		//#else
		return new ClientHandshakePacketListenerImpl(conn, mc, parent, status -> {});
		//#endif
	}

	/** Start the replay connection's login (the phase and listener are set; nothing is sent anywhere). */
	static void startLogin(Connection conn, ClientHandshakePacketListenerImpl login) {
		//#if MC >= 1.20.5
		conn.initiateServerboundPlayConnection("replay.arctic", 25565,
				net.minecraft.network.protocol.login.LoginProtocols.SERVERBOUND,
				net.minecraft.network.protocol.login.LoginProtocols.CLIENTBOUND, login, false);
		//#elif MC >= 1.20.2
		conn.initiateServerboundPlayConnection("replay.arctic", 25565, login);
		//#else
		conn.setProtocol(net.minecraft.network.ConnectionProtocol.LOGIN);
		conn.setListener(login);
		//#endif
	}

	// ---- Packets rewritten for a spectator camera -----------------------------------------

	/** The login finished packet, for the camera's profile instead of the recording player's. */
	static Packet<?> loginFinished(Packet<?> packet, GameProfile camera) {
		//#if MC >= 26.2
		return new net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket(camera,
				((net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket) packet).sessionId());
		//#elif MC >= 1.21.2
		return new net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket(camera);
		//#elif MC >= 1.20.5
		return new net.minecraft.network.protocol.login.ClientboundGameProfilePacket(camera,
				((net.minecraft.network.protocol.login.ClientboundGameProfilePacket) packet).strictErrorHandling());
		//#else
		return new net.minecraft.network.protocol.login.ClientboundGameProfilePacket(camera);
		//#endif
	}

	/** The world's login: the camera's id and spectator mode, no online checks. */
	static ClientboundLoginPacket login(ClientboundLoginPacket p, int cameraId) {
		//#if MC >= 26.2
		return new ClientboundLoginPacket(cameraId, p.hardcore(), p.levels(), p.maxPlayers(), p.chunkRadius(),
				p.simulationDistance(), p.reducedDebugInfo(), false, p.doLimitedCrafting(), spectator(p.commonPlayerSpawnInfo()),
				false, false);
		//#elif MC >= 1.20.5
		return new ClientboundLoginPacket(cameraId, p.hardcore(), p.levels(), p.maxPlayers(), p.chunkRadius(),
				p.simulationDistance(), p.reducedDebugInfo(), false, p.doLimitedCrafting(), spectator(p.commonPlayerSpawnInfo()),
				false);
		//#elif MC >= 1.20.2
		return new ClientboundLoginPacket(cameraId, p.hardcore(), p.levels(), p.maxPlayers(), p.chunkRadius(),
				p.simulationDistance(), p.reducedDebugInfo(), false, p.doLimitedCrafting(), spectator(p.commonPlayerSpawnInfo()));
		//#elif MC >= 1.20
		return new ClientboundLoginPacket(cameraId, p.hardcore(), GameType.SPECTATOR, p.previousGameType(), p.levels(),
				p.registryHolder(), p.dimensionType(), p.dimension(), p.seed(), p.maxPlayers(), p.chunkRadius(),
				p.simulationDistance(), p.reducedDebugInfo(), false, p.isDebug(), p.isFlat(), p.lastDeathLocation(),
				p.portalCooldown());
		//#elif MC >= 1.19
		return new ClientboundLoginPacket(cameraId, p.hardcore(), GameType.SPECTATOR, p.previousGameType(), p.levels(),
				p.registryHolder(), p.dimensionType(), p.dimension(), p.seed(), p.maxPlayers(), p.chunkRadius(),
				p.simulationDistance(), p.reducedDebugInfo(), false, p.isDebug(), p.isFlat(), p.lastDeathLocation());
		//#else
		// Its encoding starts with the player's id (int), hardcore (boolean) and game mode (byte).
		net.minecraft.network.FriendlyByteBuf bytes = bytesOf(p);
		bytes.setInt(0, cameraId);
		bytes.setByte(5, GameType.SPECTATOR.getId());
		//#if MC >= 1.17
		return new ClientboundLoginPacket(bytes);
		//#else
		return reread(new ClientboundLoginPacket(), bytes);
		//#endif
		//#endif
	}

	static int playerId(ClientboundLoginPacket p) {
		//#if MC >= 1.18
		return p.playerId();
		//#else
		return p.getPlayerId();
		//#endif
	}

	//#if MC < 1.19
	/** Before 1.19 packets couldn't be rebuilt field by field on every version: patch their bytes instead. */
	private static net.minecraft.network.FriendlyByteBuf bytesOf(Packet<?> p) {
		net.minecraft.network.FriendlyByteBuf bytes = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
		try {
			p.write(bytes);
		} catch (Exception e) {
			// (Writing could throw IOException before 1.17.)
			throw new IllegalStateException(e);
		}
		return bytes;
	}
	//#endif

	//#if MC < 1.17
	private static <T extends Packet<?>> T reread(T empty, net.minecraft.network.FriendlyByteBuf bytes) {
		try {
			empty.read(bytes);
		} catch (java.io.IOException e) {
			throw new IllegalStateException(e);
		}
		return empty;
	}
	//#endif

	static ClientboundRespawnPacket respawn(ClientboundRespawnPacket p) {
		//#if MC >= 1.20.2
		return new ClientboundRespawnPacket(spectator(p.commonPlayerSpawnInfo()), p.dataToKeep());
		//#else
		//#if MC >= 1.19.3
		byte keep = (byte) ((p.shouldKeep((byte) 1) ? 1 : 0) | (p.shouldKeep((byte) 2) ? 2 : 0));
		//#endif
		//#if MC >= 1.20
		return new ClientboundRespawnPacket(p.getDimensionType(), p.getDimension(), p.getSeed(), GameType.SPECTATOR,
				p.getPreviousPlayerGameType(), p.isDebug(), p.isFlat(), keep, p.getLastDeathLocation(), p.getPortalCooldown());
		//#elif MC >= 1.19.3
		return new ClientboundRespawnPacket(p.getDimensionType(), p.getDimension(), p.getSeed(), GameType.SPECTATOR,
				p.getPreviousPlayerGameType(), p.isDebug(), p.isFlat(), keep, p.getLastDeathLocation());
		//#elif MC >= 1.19
		// Before 1.19.3 there was only "keep everything" or not.
		return new ClientboundRespawnPacket(p.getDimensionType(), p.getDimension(), p.getSeed(), GameType.SPECTATOR,
				p.getPreviousPlayerGameType(), p.isDebug(), p.isFlat(), p.shouldKeepAllPlayerData(), p.getLastDeathLocation());
		//#else
		// Its encoding ends with the game mode, the previous one and three booleans.
		net.minecraft.network.FriendlyByteBuf bytes = bytesOf(p);
		bytes.setByte(bytes.writerIndex() - 5, GameType.SPECTATOR.getId());
		//#if MC >= 1.17
		return new ClientboundRespawnPacket(bytes);
		//#else
		return reread(new ClientboundRespawnPacket(), bytes);
		//#endif
		//#endif
		//#endif
	}

	//#if MC >= 1.20.2
	private static CommonPlayerSpawnInfo spectator(CommonPlayerSpawnInfo i) {
		//#if MC >= 26.3
		return new CommonPlayerSpawnInfo(i.dimensionType(), i.dimension(), i.seed(), GameType.SPECTATOR, i.previousGameType(),
				i.isDebug(), i.isFlat(), i.lastDeathLocation(), i.portalCooldown(), i.seaLevel());
		//#elif MC >= 1.21.2
		return new CommonPlayerSpawnInfo(i.dimensionType(), i.dimension(), i.seed(), GameType.SPECTATOR, i.previousGameType(),
				i.isDebug(), i.isFlat(), i.lastDeathLocation(), i.portalCooldown(), i.seaLevel());
		//#else
		return new CommonPlayerSpawnInfo(i.dimensionType(), i.dimension(), i.seed(), GameType.SPECTATOR, i.previousGameType(),
				i.isDebug(), i.isFlat(), i.lastDeathLocation(), i.portalCooldown());
		//#endif
	}
	//#endif
}
//#endif
