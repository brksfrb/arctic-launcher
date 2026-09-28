package com.arcticlauncher.client.replay;

import java.nio.ByteBuffer;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/**
 * Sorts recorded packets by their protocol names ("level_chunk_with_light",
 * "move_entity_pos"...), which stay the same across Minecraft versions: each
 * adapter only says which id is which name in each phase. Walks the stream
 * in order, following the login → configuration → play switches.
 */
public final class PacketSorter implements ReplayBackend.Classifier {
	/** Connection phases. */
	public static final int LOGIN = 0;
	public static final int CONFIGURATION = 1;
	public static final int PLAY = 2;

	private static final Map<String, Byte> CATEGORY = new HashMap<String, Byte>();
	/** Never handed to the game in a replay: they'd pop up screens, kill the camera or talk back. */
	private static final Set<String> DROPPED = new HashSet<String>();

	static {
		for (String n : new String[] {"animate", "swing_animation", "block_destruction", "block_event", "damage_event",
				"hurt_animation", "entity_event", "explode", "level_event", "level_particles", "sound", "sound_entity",
				"custom_sound", "stop_sound", "take_item_entity", "set_action_bar_text", "set_title_text",
				"set_subtitle_text", "set_titles_animation", "clear_titles", "player_chat", "system_chat",
				"disguised_chat", "delete_chat", "chat", "add_transient_block", "debug_sample", "debug_event",
				"debug_block_value", "debug_chunk_value", "debug_entity_value", "projectile_power", "block_changed_ack",
				"command_suggestions", "cooldown", "tag_query", "game_test_highlight_pos", "chunk_batch_start",
				"chunk_batch_finished", "set_title", "title", "player_combat_end", "player_combat_enter", "set_titles",
				"chat_preview", "player_chat_header", "block_break_ack"}) {
			CATEGORY.put(n, Category.TRANSIENT);
		}
		CATEGORY.put("level_chunk_with_light", Category.CHUNK);
		CATEGORY.put("level_chunk", Category.CHUNK);
		CATEGORY.put("forget_level_chunk", Category.UNLOAD);
		for (String n : new String[] {"block_update", "section_blocks_update", "chunk_blocks_update", "block_entity_data", "light_update"}) {
			CATEGORY.put(n, Category.BLOCK);
		}
		CATEGORY.put("respawn", Category.RESPAWN);
		for (String n : new String[] {"add_entity", "add_player", "add_mob", "add_painting", "add_experience_orb"}) {
			CATEGORY.put(n, Category.ENTITY_ADD);
		}
		CATEGORY.put("teleport_entity", Category.ENTITY_POS);
		CATEGORY.put("entity_position_sync", Category.ENTITY_POS);
		for (String n : new String[] {"move_entity_pos", "move_entity_pos_rot", "move_entity_rot"}) {
			CATEGORY.put(n, Category.ENTITY_MOVE);
		}
		for (String n : new String[] {"set_entity_data", "set_equipment", "update_attributes", "update_mob_effect",
				"remove_mob_effect", "rotate_head", "set_entity_motion", "set_entity_link", "set_passengers",
				"move_minecart_along_track"}) {
			CATEGORY.put(n, Category.ENTITY_OTHER);
		}
		for (String n : new String[] {"keep_alive", "ping", "disconnect", "login_disconnect", "set_health",
				"player_combat_kill", "open_screen", "open_book", "open_sign_editor", "horse_screen_open",
				"mount_screen_open", "merchant_offers", "container_close", "set_camera", "player_look_at", "move_vehicle",
				"player_abilities", "resource_pack_push", "resource_pack_pop", "resource_pack", "transfer",
				"cookie_request", "store_cookie", "show_dialog", "clear_dialog", "update_advancements",
				"select_advancements_tab", "award_stats", "recipe_book_add", "recipe_book_remove",
				"recipe_book_settings", "recipe", "place_ghost_recipe", "custom_payload", "custom_report_details",
				"server_links", "low_disk_space_warning", "player_rotation", "hello", "login_compression",
				"custom_query", "code_of_conduct", "pong_response", "container_ack", "player_combat",
				"set_display_chat_preview"}) {
			DROPPED.add(n);
			CATEGORY.put(n, Category.DROP);
		}
	}

	private final String[][] names;
	private final boolean hasConfiguration;
	private final boolean longChunkPos;
	private final boolean oldBlockPos;
	private int phase = LOGIN;

	/**
	 * {@code names[phase][id]} is the packet's name ({@link #LOGIN},
	 * {@link #CONFIGURATION}, {@link #PLAY}); {@code hasConfiguration}: the
	 * version has a configuration phase (1.20.2+); {@code longChunkPos}:
	 * forget_level_chunk carries one long, not two ints (1.20.2+).
	 */
	public PacketSorter(String[][] names, boolean hasConfiguration, boolean longChunkPos) {
		this(names, hasConfiguration, longChunkPos, false);
	}

	/** {@code oldBlockPos}: block positions pack y in the middle (1.13 and older). */
	public PacketSorter(String[][] names, boolean hasConfiguration, boolean longChunkPos, boolean oldBlockPos) {
		this.names = names;
		this.hasConfiguration = hasConfiguration;
		this.longChunkPos = longChunkPos;
		this.oldBlockPos = oldBlockPos;
	}

	/** Should the replay leave this packet out? */
	public static boolean dropped(String name) {
		return DROPPED.contains(name);
	}

	/** The category of a packet by name (outside the login and configuration phases). */
	public static byte category(String name) {
		Byte c = CATEGORY.get(name);
		return c == null ? Category.ALWAYS : c;
	}

	/**
	 * A packet name from a Mojang class name: {@code ClientboundLevelChunkWithLightPacket}
	 * → {@code level_chunk_with_light}, {@code ClientboundMoveEntityPacket$Pos} → {@code move_entity_pos}.
	 */
	public static String nameOf(String className) {
		String simple = className.substring(className.lastIndexOf('.') + 1);
		String inner = "";
		int dollar = simple.indexOf('$');
		if (dollar >= 0) {
			inner = simple.substring(dollar + 1);
			simple = simple.substring(0, dollar);
		}
		if (simple.startsWith("Clientbound")) {
			simple = simple.substring("Clientbound".length());
		}
		if (simple.endsWith("Packet")) {
			simple = simple.substring(0, simple.length() - "Packet".length());
		}
		String name = snake(simple);
		return inner.isEmpty() ? name : name + "_" + snake(inner);
	}

	private static String snake(String camel) {
		StringBuilder out = new StringBuilder();
		for (int i = 0; i < camel.length(); i++) {
			char c = camel.charAt(i);
			if (Character.isUpperCase(c)) {
				if (i > 0) {
					out.append('_');
				}
				out.append(Character.toLowerCase(c));
			} else {
				out.append(c);
			}
		}
		return out.toString();
	}

	/** The phase the next packet will be read in. */
	public int phase() {
		return phase;
	}

	/** The name of a packet (its id first), in the current phase, without moving on. */
	public String nameAt(ByteBuffer packet) {
		ByteBuffer b = packet.duplicate();
		int id = readVarInt(b);
		String[] table = names[phase];
		return id >= 0 && id < table.length ? table[id] : null;
	}

	@Override
	public byte classify(ByteBuffer packet, long[] key) {
		ByteBuffer b = packet.duplicate();
		int id = readVarInt(b);
		String[] table = names[phase];
		String name = id >= 0 && id < table.length ? table[id] : null;
		if (name == null) {
			return Category.ALWAYS;
		}
		if (phase != PLAY) {
			advance(name);
			return dropped(name) ? Category.DROP : Category.ALWAYS;
		}
		if ("start_configuration".equals(name)) {
			phase = CONFIGURATION;
			return Category.ALWAYS;
		}
		if ("login".equals(name)) {
			return Category.ALWAYS;
		}
		byte c = category(name);
		try {
			key[0] = key(c, name, b);
		} catch (RuntimeException e) {
			// A body shorter than expected: no key, treated like anything else.
			return Category.ALWAYS;
		}
		return c;
	}

	/** Follow the phase switch a packet makes. */
	private void advance(String name) {
		if (phase == LOGIN && ("login_finished".equals(name) || "game_profile".equals(name))) {
			phase = hasConfiguration ? CONFIGURATION : PLAY;
		} else if (phase == CONFIGURATION && "finish_configuration".equals(name)) {
			phase = PLAY;
		}
	}

	private long key(byte category, String name, ByteBuffer b) {
		switch (category) {
			case Category.CHUNK:
				return Category.chunk(b.getInt(), b.getInt());
			case Category.UNLOAD:
				if (longChunkPos) {
					long pos = b.getLong();
					return Category.chunk((int) pos, (int) (pos >>> 32));
				}
				return Category.chunk(b.getInt(), b.getInt());
			case Category.BLOCK:
				return blockKey(name, b);
			case Category.ENTITY_ADD:
			case Category.ENTITY_POS:
			case Category.ENTITY_MOVE:
				return readVarInt(b);
			default:
				return 0;
		}
	}

	private long blockKey(String name, ByteBuffer b) {
		if ("light_update".equals(name)) {
			int x = readVarInt(b);
			return Category.chunk(x, readVarInt(b));
		}
		if ("chunk_blocks_update".equals(name)) {
			return Category.chunk(b.getInt(), b.getInt());
		}
		long pos = b.getLong();
		if ("section_blocks_update".equals(name)) {
			// SectionPos: x 22 bits, z 22 bits, y 20 bits.
			return Category.chunk((int) (pos >> 42), (int) (pos << 22 >> 42));
		}
		int x = (int) (pos >> 38);
		// BlockPos: x 26 bits, then z 26 and y 12 (1.14+), or y 12 and z 26 (older).
		int z = oldBlockPos ? (int) (pos << 38 >> 38) : (int) (pos << 26 >> 38);
		return Category.chunk(x >> 4, z >> 4);
	}

	static int readVarInt(ByteBuffer b) {
		int value = 0;
		for (int shift = 0; shift < 35; shift += 7) {
			if (!b.hasRemaining()) {
				return -1;
			}
			byte k = b.get();
			value |= (k & 0x7F) << shift;
			if ((k & 0x80) == 0) {
				return value;
			}
		}
		return -1;
	}
}
