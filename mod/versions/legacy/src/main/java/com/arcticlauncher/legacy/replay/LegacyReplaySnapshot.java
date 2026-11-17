package com.arcticlauncher.legacy.replay;

//#if MC < 1.9
import com.arcticlauncher.client.replay.Recorder;
import com.mojang.authlib.GameProfile;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.Entity;
import net.minecraft.entity.ItemEntity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.network.Packet;
import net.minecraft.network.packet.s2c.login.LoginSuccessS2CPacket;
import net.minecraft.network.packet.s2c.play.*;
import net.minecraft.util.math.MathHelper;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.level.LevelProperties;
//#endif

/**
 * A world you are already standing in, written as the packets a server would have sent
 * to join it: login, join, position, time and weather, the chunks in view, then entities.
 * Used when recording starts without a buffer from the join (the buffer was off, or was
 * switched on later), so the saved replay still opens and plays from that moment.
 */
final class LegacyReplaySnapshot {
	private LegacyReplaySnapshot() {}

	/** Whether this version can synthesize the world (only 1.8 for now). */
	static boolean supported() {
		//#if MC < 1.9
		return true;
		//#else
		return false;
		//#endif
	}

	//#if MC < 1.9
	/** Feed the current world to {@code take}; call on the game thread. Returns the number of packets written. */
	static int write(Recorder.Take take) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ClientWorld world = mc.world;
		net.minecraft.entity.player.ClientPlayerEntity me = mc.player;
		if (world == null || me == null || mc.getNetworkHandler() == null) {
			return 0;
		}
		List<Packet<?>> out = new ArrayList<Packet<?>>();
		LevelProperties info = world.getLevelProperties();
		out.add(new LoginSuccessS2CPacket(me.getGameProfile()));
		out.add(new GameJoinS2CPacket(me.getEntityId(), mc.interactionManager.getCurrentGameMode(), info.isHardcore(),
				world.dimension.getType(), info.getDifficulty(), 20, info.getGeneratorType(), false));
		out.add(new PlayerSpawnPositionS2CPacket(world.getSpawnPos()));
		out.add(new WorldTimeUpdateS2CPacket(info.getTime(), info.getTimeOfDay(), true));
		if (world.isRaining()) {
			out.add(new GameStateChangeS2CPacket(1, 0));
			out.add(new GameStateChangeS2CPacket(7, world.getRainGradient(1)));
			out.add(new GameStateChangeS2CPacket(8, world.getThunderGradient(1)));
		}
		out.add(new PlayerAbilitiesS2CPacket(me.abilities));
		out.add(new PlayerPositionLookS2CPacket(me.x, me.y, me.z, me.yaw, me.pitch, Collections.<PlayerPositionLookS2CPacket.Flag>emptySet()));
		chunks(mc, world, me, out);
		entities(mc, world, me, out);
		int written = 0;
		for (Packet<?> packet : out) {
			byte[] bytes = LegacyReplayCodec.encode(packet);
			if (bytes != null) {
				take.packet(bytes);
				written++;
			}
		}
		return written;
	}

	private static void chunks(MinecraftClient mc, ClientWorld world, PlayerEntity me, List<Packet<?>> out) {
		int range = mc.options.viewDistance + 1;
		int cx = MathHelper.floor(me.x) >> 4;
		int cz = MathHelper.floor(me.z) >> 4;
		for (int x = cx - range; x <= cx + range; x++) {
			for (int z = cz - range; z <= cz + range; z++) {
				Chunk chunk = world.getChunk(x, z);
				if (chunk == null || chunk.isEmpty()) {
					continue;
				}
				out.add(new ChunkDataS2CPacket(chunk, true, 65535));
				for (Object entity : chunk.getBlockEntities().values()) {
					net.minecraft.block.entity.BlockEntity be = (net.minecraft.block.entity.BlockEntity) entity;
					Packet<?> update = be.getPacket();
					if (update != null) {
						out.add(update);
					}
				}
			}
		}
	}

	private static void entities(MinecraftClient mc, ClientWorld world, PlayerEntity me, List<Packet<?>> out) {
		List<Entity> all = new ArrayList<Entity>(world.loadedEntities);
		for (Entity e : all) {
			if (e == me || e.removed) {
				continue;
			}
			if (e instanceof PlayerEntity) {
				PlayerListEntry entry = mc.getNetworkHandler().getPlayerListEntry(e.getUuid());
				GameProfile profile = entry != null ? entry.getProfile() : ((PlayerEntity) e).getGameProfile();
				PlayerListS2CPacket list = new PlayerListS2CPacket(PlayerListS2CPacket.Action.ADD_PLAYER, Collections.<net.minecraft.entity.player.ServerPlayerEntity>emptyList());
				list.getEntries().add(list.new Entry(profile, 0, entry != null ? entry.getGameMode() : mc.interactionManager.getCurrentGameMode(), null));
				out.add(list);
				out.add(new PlayerSpawnS2CPacket((PlayerEntity) e));
			} else if (e instanceof LivingEntity) {
				out.add(new MobSpawnS2CPacket((LivingEntity) e));
			} else if (e instanceof ItemEntity) {
				out.add(new EntitySpawnS2CPacket(e, 2, 1));
				out.add(new EntityTrackerUpdateS2CPacket(e.getEntityId(), e.getDataTracker(), true));
				continue;
			} else {
				continue;
			}
			out.add(new EntityTrackerUpdateS2CPacket(e.getEntityId(), e.getDataTracker(), true));
			out.add(new EntitySetHeadYawS2CPacket(e, (byte) MathHelper.floor(e.getHeadRotation() * 256.0F / 360.0F)));
			LivingEntity living = (LivingEntity) e;
			net.minecraft.item.ItemStack[] armor = living.getArmorStacks();
			// Mobs list the held item first (5 slots); players list the 4 armor pieces and hold the item apart.
			boolean heldFirst = armor != null && armor.length == 5;
			if (!heldFirst) {
				out.add(new EntityEquipmentUpdateS2CPacket(e.getEntityId(), 0, living.getStackInHand()));
			}
			for (int i = 0; armor != null && i < armor.length; i++) {
				out.add(new EntityEquipmentUpdateS2CPacket(e.getEntityId(), heldFirst ? i : i + 1, armor[i]));
			}
		}
	}
	//#endif
}
