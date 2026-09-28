package com.arcticlauncher.legacy.replay;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.network.packet.s2c.login.LoginCompressionS2CPacket;
import net.minecraft.network.packet.s2c.login.LoginDisconnectS2CPacket;
import net.minecraft.network.packet.s2c.login.LoginHelloS2CPacket;
import net.minecraft.network.packet.s2c.login.LoginSuccessS2CPacket;
import net.minecraft.network.packet.s2c.play.*;

/**
 * Old packets by their modern protocol names (what the shared replay code
 * sorts by): the same meaning, whatever these versions called them.
 */
final class LegacyPacketNames {
	private static final Map<Class<?>, String> NAMES = new HashMap<Class<?>, String>();

	static {
		put(LoginHelloS2CPacket.class, "hello");
		put(LoginSuccessS2CPacket.class, "game_profile");
		put(LoginCompressionS2CPacket.class, "login_compression");
		put(LoginDisconnectS2CPacket.class, "login_disconnect");

		put(GameJoinS2CPacket.class, "login");
		put(PlayerRespawnS2CPacket.class, "respawn");
		put(KeepAliveS2CPacket.class, "keep_alive");
		put(DisconnectS2CPacket.class, "disconnect");
		put(CustomPayloadS2CPacket.class, "custom_payload");
		put(ResourcePackSendS2CPacket.class, "resource_pack");
		put(ChunkDataS2CPacket.class, "level_chunk");
		put(ChunkDeltaUpdateS2CPacket.class, "chunk_blocks_update");
		put(BlockUpdateS2CPacket.class, "block_update");
		put(BlockEntityUpdateS2CPacket.class, "block_entity_data");
		put(BlockActionS2CPacket.class, "block_event");
		put(BlockBreakingProgressS2CPacket.class, "block_destruction");
		put(EntitySpawnS2CPacket.class, "add_entity");
		put(MobSpawnS2CPacket.class, "add_mob");
		put(PlayerSpawnS2CPacket.class, "add_player");
		put(PaintingSpawnS2CPacket.class, "add_painting");
		put(ExperienceOrbSpawnS2CPacket.class, "add_experience_orb");
		put(EntityPositionS2CPacket.class, "teleport_entity");
		put(EntityS2CPacket.MoveRelative.class, "move_entity_pos");
		put(EntityS2CPacket.RotateAndMoveRelative.class, "move_entity_pos_rot");
		put(EntityS2CPacket.Rotate.class, "move_entity_rot");
		put(EntityTrackerUpdateS2CPacket.class, "set_entity_data");
		put(EntityEquipmentUpdateS2CPacket.class, "set_equipment");
		put(EntitySetHeadYawS2CPacket.class, "rotate_head");
		put(EntityVelocityUpdateS2CPacket.class, "set_entity_motion");
		put(EntityAttributesS2CPacket.class, "update_attributes");
		put(EntityStatusEffectS2CPacket.class, "update_mob_effect");
		put(RemoveEntityStatusEffectS2CPacket.class, "remove_mob_effect");
		put(EntityAttachS2CPacket.class, "set_entity_link");
		put(EntitiesDestroyS2CPacket.class, "remove_entities");
		put(EntityAnimationS2CPacket.class, "animate");
		put(EntityStatusS2CPacket.class, "entity_event");
		put(ParticleS2CPacket.class, "level_particles");
		put(WorldEventS2CPacket.class, "level_event");
		put(ExplosionS2CPacket.class, "explode");
		put(TitleS2CPacket.class, "set_title");
		put(ChatMessageS2CPacket.class, "chat");
		put(HealthUpdateS2CPacket.class, "set_health");
		put(OpenScreenS2CPacket.class, "open_screen");
		put(CloseScreenS2CPacket.class, "container_close");
		put(SignEditorOpenS2CPacket.class, "open_sign_editor");
		put(SetCameraEntityS2CPacket.class, "set_camera");
		put(StatsUpdateS2CPacket.class, "award_stats");
		put(CombatEventS2CPacket.class, "player_combat_kill");
		put(PlayerAbilitiesS2CPacket.class, "player_abilities");
		put(PlayerPositionLookS2CPacket.class, "player_position");
		put(GameStateChangeS2CPacket.class, "game_event");
		put(PlaySoundIdS2CPacket.class, "sound");
		//#if MC >= 1.9
		put(ChunkUnloadS2CPacket.class, "forget_level_chunk");
		put(PlaySoundNameS2CPacket.class, "custom_sound");
		put(SetPassengersS2CPacket.class, "set_passengers");
		put(VehicleMoveS2CPacket.class, "move_vehicle");
		//#else
		// 1.8: many chunks in one packet (nothing to key it by).
		put(ChunkMapS2CPacket.class, "map_chunk_bulk");
		//#endif
		//#if MC >= 1.12
		put(SelectAdvancementTabS2CPacket.class, "select_advancements_tab");
		put(RecipesUnlockS2CPacket.class, "recipe");
		put(CraftRecipeResponseS2CPacket.class, "place_ghost_recipe");
		//#endif
	}

	private LegacyPacketNames() {}

	private static void put(Class<?> type, String name) {
		NAMES.put(type, name);
	}

	/** The packet's modern protocol name, or "" if it isn't one the replay cares about. */
	static String of(Class<?> type) {
		String name = NAMES.get(type);
		return name == null ? "" : name;
	}
}
