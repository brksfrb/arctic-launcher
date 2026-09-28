package com.arcticlauncher.legacy.replay;

import com.arcticlauncher.client.replay.Category;
import com.arcticlauncher.client.replay.PacketSorter;
import com.arcticlauncher.client.replay.ReplayBackend;
import com.arcticlauncher.client.replay.ReplayData;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.SelfSample;
import com.mojang.authlib.GameProfile;
import io.netty.channel.embedded.EmbeddedChannel;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.network.ClientLoginNetworkHandler;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.entity.player.OtherClientPlayerEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.NetworkSide;
import net.minecraft.network.NetworkState;
import net.minecraft.network.OffThreadException;
import net.minecraft.network.Packet;
import net.minecraft.network.listener.PacketListener;
import net.minecraft.network.packet.s2c.login.LoginSuccessS2CPacket;
import net.minecraft.network.packet.s2c.play.GameJoinS2CPacket;
import net.minecraft.network.packet.s2c.play.GameStateChangeS2CPacket;
import net.minecraft.network.packet.s2c.play.PlayerRespawnS2CPacket;
import net.minecraft.network.packet.s2c.play.PlayerSpawnS2CPacket;

/**
 * Playing a replay on old versions: the same approach as newer ones (a
 * connection that goes nowhere, fed the recording on the game thread), with
 * the old game's classes.
 */
public final class LegacyReplayPlayback implements ReplayBackend {
	static final int CAMERA_ID = -1_789_435;
	private static final GameProfile CAMERA = new GameProfile(
			UUID.nameUUIDFromBytes("arctic:replay-camera".getBytes(StandardCharsets.UTF_8)), "Replay");
	private static final int BODY_STEP_MS = 50;
	private static final int BODY_JUMP_MS = 1000;
	/** Game state change reasons: game mode, credits, demo. */
	private static final int CHANGE_GAME_MODE = 3;
	private static final int WIN_GAME = 4;
	private static final int DEMO = 5;
	//#if MC >= 1.10
	private static final net.minecraft.world.GameMode SPECTATOR = net.minecraft.world.GameMode.SPECTATOR;
	private static final net.minecraft.world.GameMode SURVIVAL = net.minecraft.world.GameMode.SURVIVAL;
	//#else
	private static final net.minecraft.world.level.LevelInfo.GameMode SPECTATOR = net.minecraft.world.level.LevelInfo.GameMode.SPECTATOR;
	private static final net.minecraft.world.level.LevelInfo.GameMode SURVIVAL = net.minecraft.world.level.LevelInfo.GameMode.SURVIVAL;
	//#endif

	public static final LegacyReplayPlayback INSTANCE = new LegacyReplayPlayback();

	private ClientConnection connection;
	private EmbeddedChannel channel;
	private int selfId = -1;
	private boolean placed;
	private OtherClientPlayerEntity body;
	private int bodyTime = Integer.MIN_VALUE;
	private boolean bodyHidden;
	private Entity following;
	private boolean firstPerson;
	private ByteBuffer pixels;

	@Override
	public int protocol() {
		return LegacyReplayCodec.protocolVersion();
	}

	@Override
	public Classifier classifier(ReplayData data) {
		return LegacyReplayCodec.sorter();
	}

	@Override
	public String start(ReplayData data) {
		leaveWorld();
		return connect();
	}

	@Override
	public String restart(ReplayData data) {
		retire();
		leaveWorld();
		return connect();
	}

	private static void leaveWorld() {
		com.arcticlauncher.legacy.LegacyPlatform.leave(MinecraftClient.getInstance());
	}

	private String connect() {
		try {
			MinecraftClient mc = MinecraftClient.getInstance();
			selfId = -1;
			placed = false;
			body = null;
			bodyTime = Integer.MIN_VALUE;
			following = null;
			LegacyReplayView.fov = 0;
			ClientConnection conn = new ClientConnection(NetworkSide.CLIENTBOUND);
			((LegacyReplayConnection) conn).arctic$markReplay();
			channel = new EmbeddedChannel(conn);
			connection = conn;
			conn.setState(NetworkState.LOGIN);
			conn.setPacketListener(new ClientLoginNetworkHandler(conn, mc, new TitleScreen()));
			return null;
		} catch (RuntimeException e) {
			return "Couldn't start the replay: " + e;
		}
	}

	private void retire() {
		if (connection != null) {
			((LegacyReplayConnection) connection).arctic$retire();
		}
		if (channel != null) {
			channel.close();
		}
	}

	@Override
	@SuppressWarnings({"rawtypes", "unchecked"})
	public void apply(ByteBuffer bytes, boolean jumping) {
		if (connection == null || channel == null) {
			return;
		}
		while (channel.readOutbound() != null) {
			// Whatever the game "sent": nobody's listening.
		}
		NetworkState state = channel.attr(ClientConnection.ATTR_KEY_PROTOCOL).get();
		if (state == null) {
			return;
		}
		Packet packet;
		try {
			packet = LegacyReplayCodec.decode(state, bytes);
		} catch (IOException e) {
			throw new IllegalStateException(e);
		}
		String name = LegacyReplayCodec.name(packet);
		if (PacketSorter.dropped(name) || (jumping && PacketSorter.category(name) == Category.TRANSIENT)) {
			return;
		}
		packet = rewrite(packet, name);
		if (packet == null) {
			return;
		}
		PacketListener listener = connection.getPacketListener();
		if (listener == null) {
			return;
		}
		try {
			packet.apply(listener);
		} catch (OffThreadException ignored) {
			// Queued for the game thread by the game itself.
		}
	}

	@SuppressWarnings("rawtypes")
	private Packet rewrite(Packet packet, String name) {
		switch (name) {
			case "game_profile":
				return new LoginSuccessS2CPacket(CAMERA);
			case "login": {
				GameJoinS2CPacket p = (GameJoinS2CPacket) packet;
				selfId = p.getEntityId();
				placed = false;
				return new GameJoinS2CPacket(CAMERA_ID, SPECTATOR, p.isHardcore(), p.getChunkLoadDistance(), p.getDifficulty(),
						p.getMaxPlayers(), p.getGeneratorType(), p.hasReducedDebugInfo());
			}
			case "respawn": {
				PlayerRespawnS2CPacket p = (PlayerRespawnS2CPacket) packet;
				placed = false;
				return new PlayerRespawnS2CPacket(p.getDimensionId(), p.getDifficulty(), p.getGeneratorType(), SPECTATOR);
			}
			case "player_position":
				if (placed) {
					return null;
				}
				placed = true;
				return packet;
			case "game_event": {
				int reason = ((GameStateChangeS2CPacket) packet).getChangeType();
				return reason == CHANGE_GAME_MODE || reason == WIN_GAME || reason == DEMO ? null : packet;
			}
			case "add_player":
				return ((PlayerSpawnS2CPacket) packet).getId() == selfId ? null : packet;
			default:
				return packet;
		}
	}

	@Override
	public boolean ready() {
		MinecraftClient mc = MinecraftClient.getInstance();
		return connection != null && mc.world != null && mc.player != null
				&& connection.getPacketListener() instanceof ClientPlayNetworkHandler;
	}

	@Override
	public void stop() {
		retire();
		connection = null;
		channel = null;
		LegacyReplayView.fov = 0;
		cleanView(false);
		leaveWorld();
		MinecraftClient.getInstance().setScreen(new TitleScreen());
	}

	/** This replay's connection closed by itself. */
	public void closed(ClientConnection c) {
		if (c == connection) {
			connection = null;
			Replays.connectionClosed();
		}
	}

	// ---- The recording player's body -----------------------------------------------

	@Override
	public void showSelf(SelfSample s, boolean swing, UUID uuid, String name, boolean firstPerson) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ClientWorld world = mc.world;
		ClientPlayerEntity camera = mc.player;
		this.firstPerson = firstPerson;
		if (s == null || world == null || camera == null || selfId == -1) {
			return;
		}
		camera.inventory.selectedSlot = s.slot;
		if (firstPerson) {
			if (body != null && !bodyHidden) {
				world.removeEntity(body.getEntityId());
				bodyHidden = true;
			}
			camera.setHealth(Math.max(1, s.health));
			camera.getHungerManager().setFoodLevel(s.food);
			if (swing) {
				swingArm(camera);
			}
			return;
		}
		if (body == null || body.world != world || body.removed || bodyHidden) {
			GameProfile profile = new GameProfile(uuid != null ? uuid : UUID.randomUUID(), name != null ? name : "Player");
			body = new OtherClientPlayerEntity(world, profile);
			body.setEntityId(selfId);
			body.refreshPositionAndAngles(s.x, s.y, s.z, s.yaw, s.pitch);
			body.setHeadYaw(s.headYaw);
			body.bodyYaw = s.bodyYaw;
			world.addEntity(selfId, body);
			bodyTime = s.time;
			bodyHidden = false;
		}
		if (Math.abs(s.time - bodyTime) > BODY_JUMP_MS) {
			body.refreshPositionAndAngles(s.x, s.y, s.z, s.yaw, s.pitch);
			body.prevX = body.prevTickX = s.x;
			body.prevY = body.prevTickY = s.y;
			body.prevZ = body.prevTickZ = s.z;
			bodyTime = s.time;
		} else if (s.time - bodyTime >= BODY_STEP_MS) {
			body.updateTrackedPositionAndAngles(s.x, s.y, s.z, s.yaw, s.pitch, 3, false);
			bodyTime = s.time;
		}
		body.setHeadYaw(s.headYaw);
		body.setSneaking(s.has(SelfSample.SNEAKING));
		body.setSprinting(s.has(SelfSample.SPRINTING));
		equip(body, camera, s.slot);
		if (swing) {
			swingArm(body);
		}
	}

	private static void swingArm(PlayerEntity player) {
		//#if MC >= 1.9
		player.swingHand(net.minecraft.util.Hand.MAIN_HAND);
		//#else
		player.swingHand();
		//#endif
	}

	/** What the recording player held and wore (the camera carries their recorded inventory). */
	private static void equip(OtherClientPlayerEntity body, ClientPlayerEntity camera, int slot) {
		ItemStack held = camera.inventory.getInvStack(slot);
		//#if MC >= 1.9
		set(body, net.minecraft.entity.EquipmentSlot.MAINHAND, held);
		set(body, net.minecraft.entity.EquipmentSlot.OFFHAND, camera.getStackInHand(net.minecraft.util.Hand.OFF_HAND));
		set(body, net.minecraft.entity.EquipmentSlot.FEET, camera.inventory.getArmor(0));
		set(body, net.minecraft.entity.EquipmentSlot.LEGS, camera.inventory.getArmor(1));
		set(body, net.minecraft.entity.EquipmentSlot.CHEST, camera.inventory.getArmor(2));
		set(body, net.minecraft.entity.EquipmentSlot.HEAD, camera.inventory.getArmor(3));
		//#else
		// Slot 0 is the held item, then boots up to the helmet.
		set(body, 0, held);
		for (int i = 0; i < 4; i++) {
			set(body, i + 1, camera.inventory.getArmor(i));
		}
		//#endif
	}

	//#if MC >= 1.9
	private static void set(OtherClientPlayerEntity body, net.minecraft.entity.EquipmentSlot slot, ItemStack stack) {
		if (!ItemStack.equalsAll(body.getStack(slot), stack)) {
			body.equipStack(slot, copy(stack));
		}
	}
	//#else
	private static void set(OtherClientPlayerEntity body, int slot, ItemStack stack) {
		// setArmorSlot counts the held item as 0; getArmorSlot counts armor only.
		ItemStack now = slot == 0 ? body.inventory.getInvStack(body.inventory.selectedSlot) : body.inventory.getArmor(slot - 1);
		if (!ItemStack.equalsAll(now, stack)) {
			body.setArmorSlot(slot, copy(stack));
		}
	}
	//#endif

	private static ItemStack copy(ItemStack stack) {
		return stack == null ? null : stack.copy();
	}

	// ---- The camera ---------------------------------------------------------------

	@Override
	public void camera(double x, double y, double z, float yaw, float pitch, float fov, boolean firstPerson) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ClientPlayerEntity p = mc.player;
		if (p == null || mc.interactionManager == null) {
			return;
		}
		mc.interactionManager.setGameMode(firstPerson ? SURVIVAL : SPECTATOR);
		LegacyReplayView.fov = fov;
		if (following != null) {
			if (following.removed || following.world != mc.world) {
				follow(-1);
			} else {
				return;
			}
		}
		if (!Float.isNaN(yaw)) {
			p.yaw = p.prevYaw = yaw;
			p.pitch = p.prevPitch = pitch;
			p.setHeadYaw(yaw);
			p.bodyYaw = yaw;
		}
		double feet = y - p.getEyeHeight();
		p.updatePosition(x, feet, z);
		p.prevX = p.prevTickX = x;
		p.prevY = p.prevTickY = feet;
		p.prevZ = p.prevTickZ = z;
		p.setVelocityClient(0, 0, 0);
	}

	@Override
	public float[] look() {
		ClientPlayerEntity p = MinecraftClient.getInstance().player;
		return p == null ? new float[] {0, 0} : new float[] {p.yaw, p.pitch};
	}

	@Override
	public void follow(int entityId) {
		MinecraftClient mc = MinecraftClient.getInstance();
		Entity target = entityId < 0 || mc.world == null ? null : mc.world.getEntityById(entityId);
		following = target;
		mc.setCameraEntity(target != null ? target : mc.player);
	}

	@Override
	public List<Object[]> followable() {
		MinecraftClient mc = MinecraftClient.getInstance();
		List<Object[]> out = new ArrayList<Object[]>();
		if (mc.world == null) {
			return out;
		}
		for (PlayerEntity p : mc.world.playerEntities) {
			if (p != mc.player) {
				out.add(new Object[] {p.getEntityId(), p.getGameProfile().getName(), p.x, p.y, p.z});
			}
		}
		return out;
	}

	@Override
	public void cleanView(boolean clean) {
		MinecraftClient.getInstance().options.hudHidden = clean;
	}

	/** The frame on screen (just before it's shown), read with OpenGL. */
	@Override
	public void capture(FrameSink sink) {
		MinecraftClient mc = MinecraftClient.getInstance();
		int w = mc.width;
		int h = mc.height;
		if (pixels == null || pixels.capacity() < w * h * 4) {
			pixels = ByteBuffer.allocateDirect(w * h * 4).order(ByteOrder.nativeOrder());
		}
		pixels.clear();
		org.lwjgl.opengl.GL11.glPixelStorei(org.lwjgl.opengl.GL11.GL_PACK_ALIGNMENT, 1);
		org.lwjgl.opengl.GL11.glReadPixels(0, 0, w, h, org.lwjgl.opengl.GL11.GL_RGBA, org.lwjgl.opengl.GL11.GL_UNSIGNED_BYTE, pixels);
		pixels.rewind();
		sink.frame(w, h, pixels, true);
	}
}
