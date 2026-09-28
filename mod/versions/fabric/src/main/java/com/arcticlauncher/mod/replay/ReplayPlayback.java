//#if MC >= 1.16
package com.arcticlauncher.mod.replay;

import com.arcticlauncher.client.replay.Category;
import com.arcticlauncher.client.replay.PacketSorter;
import com.arcticlauncher.client.replay.ReplayBackend;
import com.arcticlauncher.client.replay.ReplayData;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.SelfSample;
import com.mojang.authlib.GameProfile;
import io.netty.channel.embedded.EmbeddedChannel;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.network.Connection;
import net.minecraft.network.PacketListener;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.game.ClientboundAddEntityPacket;
import net.minecraft.network.protocol.game.ClientboundGameEventPacket;
import net.minecraft.network.protocol.game.ClientboundLoginPacket;
//#if MC >= 1.19
import net.minecraft.network.protocol.game.ClientboundPlayerChatPacket;
//#endif
import net.minecraft.network.protocol.game.ClientboundRespawnPacket;
import net.minecraft.server.RunningOnDifferentThreadException;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.GameType;

/**
 * Plays a replay back on this version: a connection that goes nowhere,
 * fed the recorded packets. You watch as a spectator camera with its own id
 * and name; the recording player gets a body of their own, moved from the
 * recorded samples. Popping-up screens, your old health and teleports are
 * left out.
 */
public final class ReplayPlayback implements ReplayBackend {
	/** The camera's entity id (no server uses it). */
	static final int CAMERA_ID = -1_789_435;
	private static final GameProfile CAMERA = new GameProfile(
			UUID.nameUUIDFromBytes("arctic:replay-camera".getBytes(StandardCharsets.UTF_8)), "Replay");
	/** A new target for the body at most this often (ms of replay): like a server's updates. */
	private static final int BODY_STEP_MS = 50;
	/** Further than this between two samples is a jump, not a move. */
	private static final int BODY_JUMP_MS = 1000;
	private static final int HEAD_STEPS = 3;

	/** The one playback (registered with the core at startup). */
	public static final ReplayPlayback INSTANCE = new ReplayPlayback();

	private Connection connection;
	private EmbeddedChannel channel;
	/** Before 1.20.5: the phase the next packet is read in (later the connection knows). */
	private Object phase;
	/** The recording player's entity id in this login (-1 until the login). */
	private int selfId = -1;
	/** The first placement after each login or respawn is kept (it puts the camera in the world). */
	private boolean placed;
	private RemotePlayer body;
	private int bodyTime = Integer.MIN_VALUE;
	private boolean bodyHidden;
	private Entity following;
	private boolean firstPerson;

	@Override
	public int protocol() {
		return ReplayCodec.protocolVersion();
	}

	@Override
	public Classifier classifier(ReplayData data) {
		return ReplayCodec.sorter();
	}

	@Override
	public String start(ReplayData data) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null) {
			ReplayCompat.disconnect(ReplayCompat.message("Opening replay…"));
		}
		return connect(mc);
	}

	@Override
	public String restart(ReplayData data) {
		Minecraft mc = Minecraft.getInstance();
		retire();
		ReplayCompat.disconnect(ReplayCompat.message("Jumping…"));
		return connect(mc);
	}

	private String connect(Minecraft mc) {
		try {
			selfId = -1;
			placed = false;
			body = null;
			bodyTime = Integer.MIN_VALUE;
			following = null;
			ReplayView.fov = 0;
			Connection conn = new Connection(PacketFlow.CLIENTBOUND);
			((ReplayConnection) conn).arctic$markReplay();
			channel = new EmbeddedChannel(conn);
			connection = conn;
			ClientHandshakePacketListenerImpl login = ReplayCompat.loginListener(conn);
			phase = ReplayCodec.loginProtocol();
			ReplayCompat.startLogin(conn, login);
			return null;
		} catch (RuntimeException e) {
			return "Couldn't start the replay: " + e;
		}
	}

	/** The old connection goes quietly (its closing mustn't end the replay). */
	private void retire() {
		if (connection != null) {
			((ReplayConnection) connection).arctic$retire();
		}
		if (channel != null) {
			channel.close();
		}
	}

	@Override
	@SuppressWarnings({"unchecked", "rawtypes"})
	public void apply(ByteBuffer bytes, boolean jumping) {
		if (connection == null) {
			return;
		}
		drainOutgoing();
		Object protocol = phase != null ? phase : ((ReplayConnection) connection).arctic$inbound();
		if (protocol == null) {
			return;
		}
		Packet<?> packet = ReplayCodec.decode(protocol, bytes);
		if (phase != null) {
			phase = ReplayCodec.next(phase, packet);
		}
		ReplayCodec.beforeHandle(channel, packet);
		String name = ReplayCodec.name(packet);
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
			((Packet) packet).handle(listener);
		} catch (RunningOnDifferentThreadException ignored) {
			// Queued for the game thread by the game itself.
		}
	}

	/** Whatever the game "sent" to the replay: nobody's listening. */
	private void drainOutgoing() {
		if (channel != null) {
			channel.releaseOutbound();
		}
	}

	/** Change or leave out a packet so it suits a spectator watching (null: leave out). */
	private Packet<?> rewrite(Packet<?> packet, String name) {
		switch (name) {
			case "login_finished":
			case "game_profile":
				return ReplayCompat.loginFinished(packet, CAMERA);
			case "login": {
				ClientboundLoginPacket p = (ClientboundLoginPacket) packet;
				selfId = ReplayCompat.playerId(p);
				placed = false;
				return ReplayCompat.login(p, CAMERA_ID);
			}
			case "respawn":
				placed = false;
				return ReplayCompat.respawn((ClientboundRespawnPacket) packet);
			case "player_position":
				if (placed) {
					return null;
				}
				placed = true;
				return packet;
			case "game_event": {
				ClientboundGameEventPacket.Type type = ((ClientboundGameEventPacket) packet).getEvent();
				boolean bad = type == ClientboundGameEventPacket.CHANGE_GAME_MODE || type == ClientboundGameEventPacket.WIN_GAME
						|| type == ClientboundGameEventPacket.DEMO_EVENT;
				return bad ? null : packet;
			}
			//#if MC >= 1.19
			case "player_chat":
				ReplayCompat.systemChat(ReplayCompat.chatText((ClientboundPlayerChatPacket) packet));
				return null;
			//#endif
			case "add_entity":
				return ((ClientboundAddEntityPacket) packet).getId() == selfId ? null : packet;
			//#if MC < 1.20.2
			case "add_player":
				return ((net.minecraft.network.protocol.game.ClientboundAddPlayerPacket) packet).getEntityId() == selfId ? null : packet;
			//#endif
			default:
				return packet;
		}
	}

	@Override
	public boolean ready() {
		Minecraft mc = Minecraft.getInstance();
		return connection != null && mc.level != null && mc.player != null && connection.getPacketListener() instanceof ClientPacketListener;
	}

	@Override
	public boolean loading() {
		net.minecraft.client.gui.screens.Screen s = com.arcticlauncher.mod.Compat.screen();
		//#if MC >= 1.21.9
		return s instanceof net.minecraft.client.gui.screens.LevelLoadingScreen;
		//#else
		return s instanceof net.minecraft.client.gui.screens.LevelLoadingScreen
				|| s instanceof net.minecraft.client.gui.screens.ReceivingLevelScreen;
		//#endif
	}

	@Override
	public void stop() {
		Minecraft mc = Minecraft.getInstance();
		retire();
		connection = null;
		channel = null;
		ReplayView.fov = 0;
		cleanView(false);
		ReplayCompat.disconnect(new TitleScreen());
	}

	// ---- The recording player's body -------------------------------------------------

	@Override
	public void showSelf(SelfSample s, boolean swing, UUID uuid, String name, boolean firstPerson) {
		Minecraft mc = Minecraft.getInstance();
		ClientLevel level = mc.level;
		LocalPlayer camera = mc.player;
		this.firstPerson = firstPerson;
		if (s == null || level == null || camera == null || selfId == -1) {
			return;
		}
		ReplayCompat.selectSlot(com.arcticlauncher.mod.Compat.inventory(camera), s.slot);
		if (firstPerson) {
			hideBody(level);
			camera.setHealth(Math.max(1, s.health));
			camera.getFoodData().setFoodLevel(s.food);
			if (swing) {
				ReplayCompat.swing(camera);
			}
			return;
		}
		if (body == null || ReplayCompat.level(body) != level || com.arcticlauncher.mod.Compat.removed(body) || bodyHidden) {
			GameProfile profile = new GameProfile(uuid != null ? uuid : UUID.randomUUID(), name != null ? name : "Player");
			//#if MC >= 1.19 && MC < 1.19.3
			body = new RemotePlayer(level, profile, null);
			//#else
			body = new RemotePlayer(level, profile);
			//#endif
			body.setId(selfId);
			ReplayCompat.snap(body, s.x, s.y, s.z, s.yaw, s.pitch);
			body.setYHeadRot(s.headYaw);
			body.yBodyRot = s.bodyYaw;
			ReplayCompat.addEntity(level, body);
			bodyTime = s.time;
			bodyHidden = false;
		}
		if (Math.abs(s.time - bodyTime) > BODY_JUMP_MS) {
			ReplayCompat.snap(body, s.x, s.y, s.z, s.yaw, s.pitch);
			com.arcticlauncher.mod.Compat.settle(body);
			body.setYHeadRot(s.headYaw);
			bodyTime = s.time;
		} else if (s.time - bodyTime >= BODY_STEP_MS) {
			ReplayCompat.glide(body, s.x, s.y, s.z, s.yaw, s.pitch);
			body.lerpHeadTo(s.headYaw, HEAD_STEPS);
			bodyTime = s.time;
		}
		pose(body, s);
		equip(body, camera, s.slot);
		if (swing) {
			ReplayCompat.swing(body);
		}
	}

	private void hideBody(ClientLevel level) {
		if (body != null && !bodyHidden) {
			//#if MC >= 1.17
			level.removeEntity(body.getId(), Entity.RemovalReason.DISCARDED);
			//#else
			level.removeEntity(body.getId());
			//#endif
			bodyHidden = true;
		}
	}

	private static void pose(RemotePlayer body, SelfSample s) {
		boolean sneaking = s.has(SelfSample.SNEAKING);
		boolean swimming = s.has(SelfSample.SWIMMING);
		boolean gliding = s.has(SelfSample.FLYING_ELYTRA);
		body.setShiftKeyDown(sneaking);
		body.setSprinting(s.has(SelfSample.SPRINTING));
		body.setSwimming(swimming);
		body.setPose(gliding ? Pose.FALL_FLYING : swimming ? Pose.SWIMMING : sneaking ? Pose.CROUCHING : Pose.STANDING);
	}

	/** What the recording player held and wore: the camera carries their inventory (recorded). */
	private static void equip(RemotePlayer body, LocalPlayer camera, int slot) {
		set(body, EquipmentSlot.MAINHAND, com.arcticlauncher.mod.Compat.inventory(camera).getItem(slot));
		set(body, EquipmentSlot.OFFHAND, camera.getItemBySlot(EquipmentSlot.OFFHAND));
		set(body, EquipmentSlot.HEAD, camera.getItemBySlot(EquipmentSlot.HEAD));
		set(body, EquipmentSlot.CHEST, camera.getItemBySlot(EquipmentSlot.CHEST));
		set(body, EquipmentSlot.LEGS, camera.getItemBySlot(EquipmentSlot.LEGS));
		set(body, EquipmentSlot.FEET, camera.getItemBySlot(EquipmentSlot.FEET));
	}

	private static void set(RemotePlayer body, EquipmentSlot slot, ItemStack stack) {
		if (!ItemStack.matches(body.getItemBySlot(slot), stack)) {
			body.setItemSlot(slot, stack.copy());
		}
	}

	// ---- The camera ------------------------------------------------------------

	@Override
	public void camera(double x, double y, double z, float yaw, float pitch, float fov, boolean firstPerson) {
		Minecraft mc = Minecraft.getInstance();
		LocalPlayer p = mc.player;
		if (p == null || mc.gameMode == null) {
			return;
		}
		GameType mode = firstPerson ? GameType.SURVIVAL : GameType.SPECTATOR;
		if (mc.gameMode.getPlayerMode() != mode) {
			mc.gameMode.setLocalMode(mode);
		}
		ReplayView.fov = fov;
		if (following != null) {
			if (com.arcticlauncher.mod.Compat.removed(following) || ReplayCompat.level(following) != mc.level) {
				follow(-1);
			} else {
				return;
			}
		}
		if (!Float.isNaN(yaw)) {
			com.arcticlauncher.mod.Compat.setYRot(p, yaw);
			com.arcticlauncher.mod.Compat.setXRot(p, pitch);
			p.setYHeadRot(yaw);
			p.yBodyRot = yaw;
		}
		p.setPos(x, y - p.getEyeHeight(), z);
		com.arcticlauncher.mod.Compat.settle(p);
		p.setDeltaMovement(net.minecraft.world.phys.Vec3.ZERO);
	}

	@Override
	public float[] look() {
		LocalPlayer p = Minecraft.getInstance().player;
		return p == null ? new float[] {0, 0} : new float[] {com.arcticlauncher.mod.Compat.yRot(p), com.arcticlauncher.mod.Compat.xRot(p)};
	}

	@Override
	public void follow(int entityId) {
		Minecraft mc = Minecraft.getInstance();
		Entity target = entityId < 0 || mc.level == null ? null : mc.level.getEntity(entityId);
		following = target;
		mc.setCameraEntity(target != null ? target : mc.player);
	}

	@Override
	public List<Object[]> followable() {
		Minecraft mc = Minecraft.getInstance();
		List<Object[]> out = new ArrayList<Object[]>();
		if (mc.level == null) {
			return out;
		}
		for (AbstractClientPlayer p : mc.level.players()) {
			if (p != mc.player) {
				out.add(new Object[] {p.getId(), ReplayCompat.name(p.getGameProfile()), p.getX(), p.getY(), p.getZ()});
			}
		}
		return out;
	}

	@Override
	public void cleanView(boolean clean) {
		ReplayCompat.setHudHidden(clean);
	}

	@Override
	public void capture(FrameSink sink) {
		ReplayFrames.capture(sink);
	}

	/** This replay's connection closed by itself (not by jumping or leaving). */
	public void closed(Connection c) {
		if (c == connection) {
			connection = null;
			Replays.connectionClosed();
		}
	}
}
//#endif
