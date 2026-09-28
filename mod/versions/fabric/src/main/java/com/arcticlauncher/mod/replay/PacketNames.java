//#if MC >= 1.16 && MC < 1.20.5
package com.arcticlauncher.mod.replay;

import java.util.HashMap;
import java.util.Map;

/**
 * Packet names by class, for versions before packets carried their own
 * names (1.20.4 and older). Written from the game's class names, so they
 * match the names newer versions use (see PacketSorter).
 */
final class PacketNames {
	private static final Map<Class<?>, String> NAMES = new HashMap<Class<?>, String>();

	static {
		put(net.minecraft.network.protocol.game.ClientboundAddEntityPacket.class, "add_entity");
		put(net.minecraft.network.protocol.game.ClientboundAddExperienceOrbPacket.class, "add_experience_orb");
		put(net.minecraft.network.protocol.game.ClientboundAnimatePacket.class, "animate");
		put(net.minecraft.network.protocol.game.ClientboundAwardStatsPacket.class, "award_stats");
		put(net.minecraft.network.protocol.game.ClientboundBlockDestructionPacket.class, "block_destruction");
		put(net.minecraft.network.protocol.game.ClientboundBlockEntityDataPacket.class, "block_entity_data");
		put(net.minecraft.network.protocol.game.ClientboundBlockEventPacket.class, "block_event");
		put(net.minecraft.network.protocol.game.ClientboundBlockUpdatePacket.class, "block_update");
		put(net.minecraft.network.protocol.game.ClientboundBossEventPacket.class, "boss_event");
		put(net.minecraft.network.protocol.game.ClientboundChangeDifficultyPacket.class, "change_difficulty");
		put(net.minecraft.network.protocol.game.ClientboundCommandSuggestionsPacket.class, "command_suggestions");
		put(net.minecraft.network.protocol.game.ClientboundCommandsPacket.class, "commands");
		put(net.minecraft.network.protocol.game.ClientboundContainerClosePacket.class, "container_close");
		put(net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket.class, "container_set_content");
		put(net.minecraft.network.protocol.game.ClientboundContainerSetDataPacket.class, "container_set_data");
		put(net.minecraft.network.protocol.game.ClientboundContainerSetSlotPacket.class, "container_set_slot");
		put(net.minecraft.network.protocol.game.ClientboundCooldownPacket.class, "cooldown");
		put(net.minecraft.network.protocol.game.ClientboundEntityEventPacket.class, "entity_event");
		put(net.minecraft.network.protocol.game.ClientboundExplodePacket.class, "explode");
		put(net.minecraft.network.protocol.game.ClientboundForgetLevelChunkPacket.class, "forget_level_chunk");
		put(net.minecraft.network.protocol.game.ClientboundGameEventPacket.class, "game_event");
		put(net.minecraft.network.protocol.game.ClientboundHorseScreenOpenPacket.class, "horse_screen_open");
		put(net.minecraft.network.protocol.game.ClientboundLevelEventPacket.class, "level_event");
		put(net.minecraft.network.protocol.game.ClientboundLevelParticlesPacket.class, "level_particles");
		put(net.minecraft.network.protocol.game.ClientboundLightUpdatePacket.class, "light_update");
		put(net.minecraft.network.protocol.game.ClientboundLoginPacket.class, "login");
		put(net.minecraft.network.protocol.game.ClientboundMapItemDataPacket.class, "map_item_data");
		put(net.minecraft.network.protocol.game.ClientboundMerchantOffersPacket.class, "merchant_offers");
		put(net.minecraft.network.protocol.game.ClientboundMoveEntityPacket.class, "move_entity");
		put(net.minecraft.network.protocol.game.ClientboundMoveVehiclePacket.class, "move_vehicle");
		put(net.minecraft.network.protocol.game.ClientboundOpenBookPacket.class, "open_book");
		put(net.minecraft.network.protocol.game.ClientboundOpenScreenPacket.class, "open_screen");
		put(net.minecraft.network.protocol.game.ClientboundOpenSignEditorPacket.class, "open_sign_editor");
		put(net.minecraft.network.protocol.game.ClientboundPlaceGhostRecipePacket.class, "place_ghost_recipe");
		put(net.minecraft.network.protocol.game.ClientboundPlayerAbilitiesPacket.class, "player_abilities");
		put(net.minecraft.network.protocol.game.ClientboundPlayerLookAtPacket.class, "player_look_at");
		put(net.minecraft.network.protocol.game.ClientboundPlayerPositionPacket.class, "player_position");
		put(net.minecraft.network.protocol.game.ClientboundRecipePacket.class, "recipe");
		put(net.minecraft.network.protocol.game.ClientboundRemoveEntitiesPacket.class, "remove_entities");
		put(net.minecraft.network.protocol.game.ClientboundRemoveMobEffectPacket.class, "remove_mob_effect");
		put(net.minecraft.network.protocol.game.ClientboundRespawnPacket.class, "respawn");
		put(net.minecraft.network.protocol.game.ClientboundRotateHeadPacket.class, "rotate_head");
		put(net.minecraft.network.protocol.game.ClientboundSectionBlocksUpdatePacket.class, "section_blocks_update");
		put(net.minecraft.network.protocol.game.ClientboundSelectAdvancementsTabPacket.class, "select_advancements_tab");
		put(net.minecraft.network.protocol.game.ClientboundSetCameraPacket.class, "set_camera");
		put(net.minecraft.network.protocol.game.ClientboundSetCarriedItemPacket.class, "set_carried_item");
		put(net.minecraft.network.protocol.game.ClientboundSetChunkCacheCenterPacket.class, "set_chunk_cache_center");
		put(net.minecraft.network.protocol.game.ClientboundSetChunkCacheRadiusPacket.class, "set_chunk_cache_radius");
		put(net.minecraft.network.protocol.game.ClientboundSetDefaultSpawnPositionPacket.class, "set_default_spawn_position");
		put(net.minecraft.network.protocol.game.ClientboundSetDisplayObjectivePacket.class, "set_display_objective");
		put(net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket.class, "set_entity_data");
		put(net.minecraft.network.protocol.game.ClientboundSetEntityLinkPacket.class, "set_entity_link");
		put(net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket.class, "set_entity_motion");
		put(net.minecraft.network.protocol.game.ClientboundSetEquipmentPacket.class, "set_equipment");
		put(net.minecraft.network.protocol.game.ClientboundSetExperiencePacket.class, "set_experience");
		put(net.minecraft.network.protocol.game.ClientboundSetHealthPacket.class, "set_health");
		put(net.minecraft.network.protocol.game.ClientboundSetObjectivePacket.class, "set_objective");
		put(net.minecraft.network.protocol.game.ClientboundSetPassengersPacket.class, "set_passengers");
		put(net.minecraft.network.protocol.game.ClientboundSetPlayerTeamPacket.class, "set_player_team");
		put(net.minecraft.network.protocol.game.ClientboundSetScorePacket.class, "set_score");
		put(net.minecraft.network.protocol.game.ClientboundSetTimePacket.class, "set_time");
		put(net.minecraft.network.protocol.game.ClientboundSoundEntityPacket.class, "sound_entity");
		put(net.minecraft.network.protocol.game.ClientboundSoundPacket.class, "sound");
		put(net.minecraft.network.protocol.game.ClientboundStopSoundPacket.class, "stop_sound");
		put(net.minecraft.network.protocol.game.ClientboundTabListPacket.class, "tab_list");
		put(net.minecraft.network.protocol.game.ClientboundTagQueryPacket.class, "tag_query");
		put(net.minecraft.network.protocol.game.ClientboundTakeItemEntityPacket.class, "take_item_entity");
		put(net.minecraft.network.protocol.game.ClientboundTeleportEntityPacket.class, "teleport_entity");
		put(net.minecraft.network.protocol.game.ClientboundUpdateAdvancementsPacket.class, "update_advancements");
		put(net.minecraft.network.protocol.game.ClientboundUpdateAttributesPacket.class, "update_attributes");
		put(net.minecraft.network.protocol.game.ClientboundUpdateMobEffectPacket.class, "update_mob_effect");
		put(net.minecraft.network.protocol.game.ClientboundUpdateRecipesPacket.class, "update_recipes");
		put(net.minecraft.network.protocol.login.ClientboundCustomQueryPacket.class, "custom_query");
		put(net.minecraft.network.protocol.login.ClientboundGameProfilePacket.class, "game_profile");
		put(net.minecraft.network.protocol.login.ClientboundHelloPacket.class, "hello");
		put(net.minecraft.network.protocol.login.ClientboundLoginCompressionPacket.class, "login_compression");
		put(net.minecraft.network.protocol.login.ClientboundLoginDisconnectPacket.class, "login_disconnect");
		//#if MC >= 1.17
		put(net.minecraft.network.protocol.game.ClientboundClearTitlesPacket.class, "clear_titles");
		put(net.minecraft.network.protocol.game.ClientboundInitializeBorderPacket.class, "initialize_border");
		put(net.minecraft.network.protocol.game.ClientboundPlayerCombatEndPacket.class, "player_combat_end");
		put(net.minecraft.network.protocol.game.ClientboundPlayerCombatEnterPacket.class, "player_combat_enter");
		put(net.minecraft.network.protocol.game.ClientboundPlayerCombatKillPacket.class, "player_combat_kill");
		put(net.minecraft.network.protocol.game.ClientboundSetActionBarTextPacket.class, "set_action_bar_text");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderCenterPacket.class, "set_border_center");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderLerpSizePacket.class, "set_border_lerp_size");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderSizePacket.class, "set_border_size");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderWarningDelayPacket.class, "set_border_warning_delay");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderWarningDistancePacket.class, "set_border_warning_distance");
		put(net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket.class, "set_subtitle_text");
		put(net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket.class, "set_title_text");
		put(net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket.class, "set_titles_animation");
		//#endif
		//#if MC >= 1.18
		put(net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket.class, "level_chunk_with_light");
		put(net.minecraft.network.protocol.game.ClientboundSetSimulationDistancePacket.class, "set_simulation_distance");
		//#endif
		//#if MC >= 1.19
		put(net.minecraft.network.protocol.game.ClientboundBlockChangedAckPacket.class, "block_changed_ack");
		put(net.minecraft.network.protocol.game.ClientboundCustomChatCompletionsPacket.class, "custom_chat_completions");
		put(net.minecraft.network.protocol.game.ClientboundDeleteChatPacket.class, "delete_chat");
		put(net.minecraft.network.protocol.game.ClientboundPlayerChatPacket.class, "player_chat");
		put(net.minecraft.network.protocol.game.ClientboundServerDataPacket.class, "server_data");
		put(net.minecraft.network.protocol.game.ClientboundSystemChatPacket.class, "system_chat");
		//#endif
		//#if MC >= 1.19.3
		put(net.minecraft.network.protocol.game.ClientboundDisguisedChatPacket.class, "disguised_chat");
		put(net.minecraft.network.protocol.game.ClientboundPlayerInfoRemovePacket.class, "player_info_remove");
		put(net.minecraft.network.protocol.game.ClientboundPlayerInfoUpdatePacket.class, "player_info_update");
		//#endif
		//#if MC >= 1.19.4
		put(net.minecraft.network.protocol.game.ClientboundBundlePacket.class, "bundle");
		put(net.minecraft.network.protocol.game.ClientboundChunksBiomesPacket.class, "chunks_biomes");
		put(net.minecraft.network.protocol.game.ClientboundDamageEventPacket.class, "damage_event");
		put(net.minecraft.network.protocol.game.ClientboundHurtAnimationPacket.class, "hurt_animation");
		put(net.minecraft.network.protocol.game.ClientboundLevelChunkPacketData.class, "level_chunk_packet_data");
		put(net.minecraft.network.protocol.game.ClientboundLightUpdatePacketData.class, "light_update_packet_data");
		//#endif
		// Only on older versions.
		//#if MC < 1.19
		put(net.minecraft.network.protocol.game.ClientboundAddMobPacket.class, "add_mob");
		put(net.minecraft.network.protocol.game.ClientboundAddPaintingPacket.class, "add_painting");
		put(net.minecraft.network.protocol.game.ClientboundBlockBreakAckPacket.class, "block_break_ack");
		put(net.minecraft.network.protocol.game.ClientboundChatPacket.class, "chat");
		//#endif
		//#if MC < 1.19.3
		put(net.minecraft.network.protocol.game.ClientboundCustomSoundPacket.class, "custom_sound");
		put(net.minecraft.network.protocol.game.ClientboundPlayerInfoPacket.class, "player_info");
		//#endif
		//#if MC < 1.17
		put(net.minecraft.network.protocol.game.ClientboundContainerAckPacket.class, "container_ack");
		put(net.minecraft.network.protocol.game.ClientboundPlayerCombatPacket.class, "player_combat");
		put(net.minecraft.network.protocol.game.ClientboundSetBorderPacket.class, "set_border");
		put(net.minecraft.network.protocol.game.ClientboundSetTitlesPacket.class, "set_titles");
		//#endif
		//#if MC < 1.18
		put(net.minecraft.network.protocol.game.ClientboundLevelChunkPacket.class, "level_chunk");
		//#endif
		//#if MC >= 1.17 && MC < 1.19
		put(net.minecraft.network.protocol.game.ClientboundAddVibrationSignalPacket.class, "add_vibration_signal");
		//#endif
		//#if MC >= 1.19 && MC < 1.19.3
		put(net.minecraft.network.protocol.game.ClientboundChatPreviewPacket.class, "chat_preview");
		put(net.minecraft.network.protocol.game.ClientboundPlayerChatHeaderPacket.class, "player_chat_header");
		put(net.minecraft.network.protocol.game.ClientboundSetDisplayChatPreviewPacket.class, "set_display_chat_preview");
		//#endif
		//#if MC >= 1.20.2
		put(net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket.class, "custom_payload");
		put(net.minecraft.network.protocol.common.ClientboundDisconnectPacket.class, "disconnect");
		put(net.minecraft.network.protocol.common.ClientboundKeepAlivePacket.class, "keep_alive");
		put(net.minecraft.network.protocol.common.ClientboundPingPacket.class, "ping");
		//#if MC >= 1.20.3
		put(net.minecraft.network.protocol.common.ClientboundResourcePackPopPacket.class, "resource_pack_pop");
		put(net.minecraft.network.protocol.common.ClientboundResourcePackPushPacket.class, "resource_pack_push");
		put(net.minecraft.network.protocol.game.ClientboundResetScorePacket.class, "reset_score");
		put(net.minecraft.network.protocol.game.ClientboundTickingStatePacket.class, "ticking_state");
		put(net.minecraft.network.protocol.game.ClientboundTickingStepPacket.class, "ticking_step");
		//#else
		put(net.minecraft.network.protocol.common.ClientboundResourcePackPacket.class, "resource_pack");
		//#endif
		put(net.minecraft.network.protocol.common.ClientboundUpdateTagsPacket.class, "update_tags");
		put(net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket.class, "finish_configuration");
		put(net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket.class, "registry_data");
		put(net.minecraft.network.protocol.configuration.ClientboundUpdateEnabledFeaturesPacket.class, "update_enabled_features");
		put(net.minecraft.network.protocol.game.ClientboundChunkBatchFinishedPacket.class, "chunk_batch_finished");
		put(net.minecraft.network.protocol.game.ClientboundChunkBatchStartPacket.class, "chunk_batch_start");
		put(net.minecraft.network.protocol.game.ClientboundStartConfigurationPacket.class, "start_configuration");
		//#else
		put(net.minecraft.network.protocol.game.ClientboundAddPlayerPacket.class, "add_player");
		put(net.minecraft.network.protocol.game.ClientboundCustomPayloadPacket.class, "custom_payload");
		put(net.minecraft.network.protocol.game.ClientboundDisconnectPacket.class, "disconnect");
		put(net.minecraft.network.protocol.game.ClientboundKeepAlivePacket.class, "keep_alive");
		//#if MC >= 1.17
		put(net.minecraft.network.protocol.game.ClientboundPingPacket.class, "ping");
		//#endif
		put(net.minecraft.network.protocol.game.ClientboundResourcePackPacket.class, "resource_pack");
		//#if MC >= 1.19.3
		put(net.minecraft.network.protocol.game.ClientboundUpdateEnabledFeaturesPacket.class, "update_enabled_features");
		//#endif
		put(net.minecraft.network.protocol.game.ClientboundUpdateTagsPacket.class, "update_tags");
		//#endif
	}

	private PacketNames() {}

	private static void put(Class<?> type, String name) {
		NAMES.put(type, name);
	}

	/** The packet's protocol name, or "" if it isn't one we know. */
	static String of(Class<?> type) {
		String name = NAMES.get(type);
		return name == null ? "" : name;
	}
}
//#endif
