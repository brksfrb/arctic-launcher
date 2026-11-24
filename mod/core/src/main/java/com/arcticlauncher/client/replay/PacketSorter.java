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
	private static final Map<String, Integer> STATE = new HashMap<String, Integer>();
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
		// 1.8: how a server sends nearly every chunk. Untracked, a long session's clips start with all of them.
		CATEGORY.put("map_chunk_bulk", Category.CHUNK_BULK);
		CATEGORY.put("forget_level_chunk", Category.UNLOAD);
		for (String n : new String[] {"block_update", "section_blocks_update", "chunk_blocks_update", "block_entity_data", "light_update"}) {
			CATEGORY.put(n, Category.BLOCK);
		}
		CATEGORY.put("block_update", Category.BLOCK_UPDATE);
		for (String name : new String[] {"set_time", "set_experience", "set_carried_item", "tab_list",
				"set_border_center", "set_border_warning_delay", "set_border_warning_distance"}) {
			STATE.put(name, STATE.size() + 1);
			CATEGORY.put(name, Category.STATE);
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
		CATEGORY.put("remove_entities", Category.ENTITY_REMOVE);
		CATEGORY.put("remove_entity", Category.ENTITY_REMOVE);
		CATEGORY.put("rotate_head", Category.ENTITY_HEAD);
		CATEGORY.put("set_entity_motion", Category.ENTITY_MOTION);
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
	/** 1.8 layout: chunks come in bulk, and a level_chunk with no sections unloads one. */
	private final boolean bulkChunks;
	private final int protocol;
	private final Map<java.util.List<String>, Long> scores = new HashMap<java.util.List<String>, Long>();
	private final Map<java.util.List<Long>, Long> bosses = new HashMap<java.util.List<Long>, Long>();

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
		this(names, hasConfiguration, longChunkPos, oldBlockPos, 0);
	}

	public PacketSorter(String[][] names, boolean hasConfiguration, boolean longChunkPos, boolean oldBlockPos, int protocol) {
		this.names = names;
		this.protocol = protocol;
		this.hasConfiguration = hasConfiguration;
		this.longChunkPos = longChunkPos;
		this.oldBlockPos = oldBlockPos;
		this.bulkChunks = names.length > PLAY && java.util.Arrays.asList(names[PLAY]).contains("map_chunk_bulk");
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
			if ("set_entity_data".equals(name)) {
				int entity = readVarInt(b);
				int mask = bulkChunks ? legacyMetadataFields(b) : metadataFields(b);
				if (mask == 0) {
					key[0] = entity;
					return Category.ENTITY_OTHER;
				}
				key[0] = ((long) entity << 32) | (mask & 0xFFFFFFFFL);
				return Category.ENTITY_DATA;
			}
			if ("game_event".equals(name)) {
				int event = b.get() & 255;
				if (event == 1 || event == 2) key[0] = 16; // Start and stop rain set the same flag.
				else if (event == 7 || event == 8) key[0] = 16 + event;
				else return c;
				return Category.STATE;
			}
			if ("boss_event".equals(name)) {
				long most = b.getLong();
				long least = b.getLong();
				int action = readVarInt(b);
				if (action < 2 || action > 5) return c;
				key[0] = 0x100000000L + bosses.computeIfAbsent(java.util.Arrays.asList(most, least, (long) action), k -> (long) bosses.size());
				return Category.STATE;
			}
			if ("world_border".equals(name) && protocol > 0 && protocol < 755) {
				int action = readVarInt(b);
				if (action != 2 && action != 4 && action != 5) return c;
				key[0] = 32 + action;
				return Category.STATE;
			}
			if ("set_score".equals(name) && protocol > 0) {
				String owner = readString(b);
				if (protocol < 765 && readVarInt(b) != 0) return c;
				String objective = readString(b);
				key[0] = 0x200000000L + scores.computeIfAbsent(java.util.Arrays.asList(owner, objective), k -> (long) scores.size());
				return Category.STATE;
			}
			key[0] = key(c, name, b);
			if (c == Category.CHUNK && bulkChunks && b.remaining() >= 3 && b.get() != 0 && b.getShort() == 0) {
				// Full chunk, no sections: 1.8's way of unloading it.
				c = Category.UNLOAD;
			}
		} catch (RuntimeException e) {
			// A body shorter than expected: no key, treated like anything else.
			return Category.ALWAYS;
		}
		return c;
	}

	@Override
	public int[] removedEntities(ByteBuffer packet) {
		String name = nameAt(packet);
		if (!"remove_entities".equals(name) && !"remove_entity".equals(name)) return null;
		try {
			ByteBuffer b = packet.duplicate();
			readVarInt(b);
			int count = "remove_entity".equals(name) ? 1 : readVarInt(b);
			if (count < 0 || count > b.remaining()) return null;
			int[] ids = new int[count];
			for (int i = 0; i < count; i++) {
				// A truncated or oversized VarInt must not retire an unrelated entity.
				int value = 0;
				boolean complete = false;
				for (int shift = 0; shift < 35; shift += 7) {
					int k = b.get() & 255;
					if (shift == 28 && (k & 240) != 0) return null;
					value |= (k & 127) << shift;
					if (k < 128) { complete = true; break; }
				}
				if (!complete) return null;
				ids[i] = value;
			}
			return b.hasRemaining() ? null : ids;
		} catch (RuntimeException e) { return null; }
	}

	/**
	 * 1.8's map_chunk_bulk: skylight flag, chunk count, then each chunk's x, z (ints) and section
	 * mask (short) ahead of the data. Null if it doesn't read that way.
	 */
	@Override
	public long[] bulkChunks(ByteBuffer packet) {
		if (!"map_chunk_bulk".equals(nameAt(packet))) return null;
		try {
			ByteBuffer b = packet.duplicate();
			readVarInt(b);
			b.get();
			int count = readVarInt(b);
			if (count < 0 || count * 10L > b.remaining()) return null;
			long[] chunks = new long[count];
			for (int i = 0; i < count; i++) {
				int x = b.getInt();
				int z = b.getInt();
				b.getShort();
				chunks[i] = Category.chunk(x, z);
			}
			return chunks;
		} catch (RuntimeException e) {
			return null;
		}
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
			case Category.BLOCK_UPDATE: {
				long pos = b.getLong();
				return oldBlockPos ? (pos & 0xffffffc000000000L) | ((pos & 0x3ffffffL) << 12) | ((pos >>> 26) & 0xfff) : pos;
			}
			case Category.STATE:
				return STATE.get(name);
			case Category.ENTITY_ADD:
			case Category.ENTITY_POS:
			case Category.ENTITY_MOVE:
			case Category.ENTITY_HEAD:
			case Category.ENTITY_MOTION:
				return readVarInt(b);
			case Category.ENTITY_OTHER:
				// Attachment packets have version-specific layouts; keep them conservatively.
				return "set_entity_link".equals(name) ? Long.MIN_VALUE : readVarInt(b);
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

	private static String readString(ByteBuffer b) {
		int size = readVarInt(b);
		if (size < 0 || size > b.remaining() || size > 32767 * 3) throw new IllegalArgumentException("invalid packet string");
		byte[] text = new byte[size];
		b.get(text);
		return new String(text, java.nio.charset.StandardCharsets.UTF_8);
	}

	/** 1.8's packed type/index and 0x7f terminator. Preserve packets carrying complex item tags. */
	private static int legacyMetadataFields(ByteBuffer b) {
		int mask = 0;
		while (b.hasRemaining()) {
			int header = b.get() & 255;
			if (header == 127) return b.hasRemaining() ? 0 : mask;
			int bytes;
			switch (header >>> 5) {
				case 0: bytes = 1; break;
				case 1: bytes = 2; break;
				case 2:
				case 3: bytes = 4; break;
				case 4: bytes = readVarInt(b); break;
				case 5:
					if (b.getShort() < 0) bytes = 0;
					else {
						b.get(); b.getShort();
						if (b.get() != 0) return 0;
						bytes = 0;
					}
					break;
				case 6:
				case 7: bytes = 12; break;
				default: return 0;
			}
			if (bytes < 0 || bytes > b.remaining()) return 0;
			b.position(b.position() + bytes);
			mask |= 1 << (header & 31);
		}
		return 0;
	}

	/** Shared primitive serializers. Registry-dependent and unfamiliar values stay intact. */
	private int metadataFields(ByteBuffer b) {
		if (protocol == 0) return 0;
		int mask = 0;
		while (b.hasRemaining()) {
			int index = b.get() & 255;
			if (index == 255) return b.hasRemaining() ? 0 : mask;
			if (index >= 32) return 0;
			int type = readVarInt(b);
			if (type == 0) b.get();
			else if (type == 1 || (type == 2 && protocol >= 761)) {
				int max = type == 1 ? 5 : 10;
				while (b.get() < 0) if (--max == 0) return 0;
			} else if (type == (protocol >= 761 ? 3 : 2)) b.getFloat();
			else return 0;
			mask |= 1 << index;
		}
		return 0;
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
