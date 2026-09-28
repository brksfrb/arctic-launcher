package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

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
}
