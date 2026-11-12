package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.nio.ByteBuffer;
import org.junit.jupiter.api.Test;

class PacketSorterTest {
	private static final String[] LOGIN = {"hello", "login_finished"};
	private static final String[] CONFIG = {"registry_data", "finish_configuration"};
	private static final String[] PLAY = {"login", "level_chunk_with_light", "forget_level_chunk", "block_update",
			"move_entity_pos", "sound", "keep_alive", "section_blocks_update", "light_update", "start_configuration", "respawn"};

	private static PacketSorter sorter() {
		return new PacketSorter(new String[][] {LOGIN, CONFIG, PLAY}, true, true);
	}

	private static ByteBuffer packet(int id, Object... fields) {
		ByteBuffer b = ByteBuffer.allocate(64);
		varInt(b, id);
		for (Object f : fields) {
			if (f instanceof Integer) {
				b.putInt((Integer) f);
			} else if (f instanceof Long) {
				b.putLong((Long) f);
			} else if (f instanceof String) {
				varInt(b, Integer.parseInt((String) f));
			}
		}
		b.flip();
		return b;
	}

	private static void varInt(ByteBuffer b, int v) {
		while ((v & ~0x7F) != 0) {
			b.put((byte) ((v & 0x7F) | 0x80));
			v >>>= 7;
		}
		b.put((byte) v);
	}

	private static long blockPos(int x, int y, int z) {
		return ((long) (x & 0x3FFFFFF) << 38) | ((long) (z & 0x3FFFFFF) << 12) | (y & 0xFFF);
	}

	@Test
	void followsTheLoginAndConfigurationPhases() {
		PacketSorter s = sorter();
		long[] key = new long[1];
		assertEquals(Category.DROP, s.classify(packet(0), key));
		assertEquals(PacketSorter.LOGIN, s.phase());
		assertEquals(Category.ALWAYS, s.classify(packet(1), key));
		assertEquals(PacketSorter.CONFIGURATION, s.phase());
		assertEquals(Category.ALWAYS, s.classify(packet(0), key));
		s.classify(packet(1), key);
		assertEquals(PacketSorter.PLAY, s.phase());
		assertEquals(Category.ALWAYS, s.classify(packet(0), key));
		s.classify(packet(9), key);
		assertEquals(PacketSorter.CONFIGURATION, s.phase());
	}

	private static PacketSorter playing() {
		PacketSorter s = sorter();
		long[] key = new long[1];
		s.classify(packet(1), key);
		s.classify(packet(1), key);
		return s;
	}

	@Test
	void keysChunksBlocksAndEntities() {
		PacketSorter s = playing();
		long[] key = new long[1];
		assertEquals(Category.CHUNK, s.classify(packet(1, 3, -4), key));
		assertEquals(Category.chunk(3, -4), key[0]);
		long chunkPos = (3 & 0xFFFFFFFFL) | ((long) -4 << 32);
		assertEquals(Category.UNLOAD, s.classify(packet(2, chunkPos), key));
		assertEquals(Category.chunk(3, -4), key[0]);
		assertEquals(Category.BLOCK, s.classify(packet(3, blockPos(50, 70, -70)), key));
		assertEquals(Category.chunk(3, -5), key[0]);
		assertEquals(Category.ENTITY_MOVE, s.classify(packet(4, "300"), key));
		assertEquals(300, key[0]);
		long section = ((long) (3 & 0x3FFFFF) << 42) | ((long) (-4 & 0x3FFFFF) << 20) | 4;
		assertEquals(Category.BLOCK, s.classify(packet(7, section), key));
		assertEquals(Category.chunk(3, -4), key[0]);
		assertEquals(Category.BLOCK, s.classify(packet(8, "7", "9"), key));
		assertEquals(Category.chunk(7, 9), key[0]);
		assertEquals(Category.TRANSIENT, s.classify(packet(5), key));
		assertEquals(Category.DROP, s.classify(packet(6), key));
		assertEquals(Category.RESPAWN, s.classify(packet(10), key));
	}

	@Test
	void unknownIdsAndShortBodiesAreJustKept() {
		PacketSorter s = playing();
		long[] key = new long[1];
		assertEquals(Category.ALWAYS, s.classify(packet(99), key));
		assertEquals(Category.ALWAYS, s.classify(packet(1, 5), key));
	}

	@Test
	void namesComeFromMojangClassNames() {
		assertEquals("level_chunk_with_light", PacketSorter.nameOf("net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket"));
		assertEquals("move_entity_pos_rot", PacketSorter.nameOf("net.minecraft.network.protocol.game.ClientboundMoveEntityPacket$PosRot"));
		assertEquals("game_profile", PacketSorter.nameOf("ClientboundGameProfilePacket"));
	}

	@Test
	void screensAndKeepAlivesAreDropped() {
		assertTrue(PacketSorter.dropped("open_screen"));
		assertTrue(PacketSorter.dropped("keep_alive"));
		assertFalse(PacketSorter.dropped("level_chunk_with_light"));
	}

	@Test
	void readsOldBlockPositions() {
		PacketSorter s = new PacketSorter(new String[][] {{"login_finished"}, {}, PLAY}, false, false, true);
		long[] key = new long[1];
		s.classify(packet(0), key);
		long old = ((long) (50 & 0x3FFFFFF) << 38) | ((long) (70 & 0xFFF) << 26) | (-70 & 0x3FFFFFF);
		assertEquals(Category.BLOCK, s.classify(packet(3, old), key));
		assertEquals(Category.chunk(3, -5), key[0]);
	}

	@Test
	void readsEntityLifecycleAndIndependentStateKeys() {
		PacketSorter s = new PacketSorter(new String[][] {{"login_finished"}, {},
				{"remove_entities", "remove_entity", "rotate_head", "set_entity_motion", "set_entity_data", "set_entity_link"}}, false, false);
		long[] key = new long[1]; s.classify(packet(0), key);
		assertEquals(Category.ENTITY_REMOVE, s.classify(packet(0, "2", "7", "300"), key));
		assertArrayEquals(new int[] {7, 300}, s.removedEntities(packet(0, "2", "7", "300")));
		assertArrayEquals(new int[] {300}, s.removedEntities(packet(1, "300")));
		assertEquals(Category.ENTITY_HEAD, s.classify(packet(2, "300"), key)); assertEquals(300, key[0]);
		assertEquals(Category.ENTITY_MOTION, s.classify(packet(3, "7"), key)); assertEquals(7, key[0]);
		assertEquals(Category.ENTITY_OTHER, s.classify(packet(4, "9"), key)); assertEquals(9, key[0]);
		assertEquals(Category.ENTITY_OTHER, s.classify(packet(5, 9), key)); assertEquals(Long.MIN_VALUE, key[0]);
		assertNull(s.removedEntities(packet(0, "2", "7")));
		assertNull(s.removedEntities(ByteBuffer.wrap(new byte[] {0, 1, (byte) 0x80})));
		assertNull(s.removedEntities(ByteBuffer.wrap(new byte[] {0, 1, (byte) 0xff, (byte) 0xff, (byte) 0xff, (byte) 0xff, 0x7f})));
		assertNull(s.removedEntities(packet(2, "7")));
	}

	@Test
	void readsTheChunksOfAnOldChunkBulk() {
		PacketSorter s = new PacketSorter(new String[][] {{"game_profile"}, {}, {"login", "map_chunk_bulk", "level_chunk"}}, false, false, true);
		ByteBuffer bulk = ByteBuffer.allocate(64);
		bulk.put((byte) 1).put((byte) 1).put((byte) 2).putInt(3).putInt(-4).putShort((short) 0xFF).putInt(5).putInt(6).putShort((short) 1);
		bulk.put(new byte[] {9, 9, 9}).flip();
		s.classify(packet(0), new long[1]); // Logged in.
		assertEquals(Category.CHUNK_BULK, s.classify(bulk, new long[1]));
		assertArrayEquals(new long[] {Category.chunk(3, -4), Category.chunk(5, 6)}, s.bulkChunks(bulk));
		// A body too short for its count isn't trusted.
		assertNull(s.bulkChunks(ByteBuffer.wrap(new byte[] {1, 1, 5, 0, 0, 0, 1})));
		assertNull(s.bulkChunks(packet(0)));
		// 1.8 unloads a chunk with an empty full-chunk level_chunk.
		ByteBuffer unload = ByteBuffer.allocate(13);
		unload.put((byte) 2).putInt(3).putInt(-4).put((byte) 1).putShort((short) 0).put((byte) 0).flip();
		long[] key = new long[1];
		assertEquals(Category.UNLOAD, s.classify(unload, key));
		assertEquals(Category.chunk(3, -4), key[0]);
		ByteBuffer load = ByteBuffer.allocate(13);
		load.put((byte) 2).putInt(3).putInt(-4).put((byte) 1).putShort((short) 1).put((byte) 0).flip();
		assertEquals(Category.CHUNK, s.classify(load, key));
	}
}
