package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class SeekPlanTest {
	private static final long A = Category.chunk(0, 0);
	private static final long B = Category.chunk(1, 0);

	private static int[] plan(byte[] cats, long[] keys) {
		return SeekPlan.plan(cats, keys, 0, cats.length);
	}

	@Test
	void keepsOnlyTheLastCopyOfEachChunkAndBlockChangesAfterIt() {
		byte[] cats = {Category.CHUNK, Category.BLOCK, Category.CHUNK, Category.BLOCK, Category.CHUNK};
		long[] keys = {A, A, A, A, B};
		assertArrayEquals(new int[] {2, 3, 4}, plan(cats, keys));
	}

	@Test
	void anUnloadedChunkLeavesOnlyTheUnload() {
		byte[] cats = {Category.CHUNK, Category.BLOCK, Category.UNLOAD};
		long[] keys = {A, A, A};
		assertArrayEquals(new int[] {2}, plan(cats, keys));
	}

	@Test
	void movesBeforeTheLastTeleportAreSkipped() {
		byte[] cats = {Category.ENTITY_ADD, Category.ENTITY_MOVE, Category.ENTITY_POS, Category.ENTITY_MOVE, Category.ENTITY_POS, Category.ENTITY_MOVE};
		long[] keys = {7, 7, 7, 7, 7, 7};
		assertArrayEquals(new int[] {0, 4, 5}, plan(cats, keys));
	}

	@Test
	void otherEntitiesAreIndependent() {
		byte[] cats = {Category.ENTITY_MOVE, Category.ENTITY_POS, Category.ENTITY_MOVE};
		long[] keys = {1, 2, 1};
		assertArrayEquals(new int[] {0, 1, 2}, plan(cats, keys));
	}

	@Test
	void aRespawnDropsTheWorldBeforeItButKeepsLastingState() {
		byte[] cats = {Category.CHUNK, Category.ALWAYS, Category.ENTITY_ADD, Category.RESPAWN, Category.CHUNK, Category.RESPAWN, Category.CHUNK};
		long[] keys = {A, 0, 3, 0, B, 0, A};
		assertArrayEquals(new int[] {1, 5, 6}, plan(cats, keys));
	}

	@Test
	void transientAndDroppedPacketsAreSkipped() {
		byte[] cats = {Category.TRANSIENT, Category.DROP, Category.ALWAYS};
		long[] keys = {0, 0, 0};
		assertArrayEquals(new int[] {2}, plan(cats, keys));
	}

	@Test
	void honorsTheWindow() {
		byte[] cats = {Category.ALWAYS, Category.ALWAYS, Category.ALWAYS, Category.ALWAYS};
		long[] keys = {0, 0, 0, 0};
		assertArrayEquals(new int[] {1, 2}, SeekPlan.plan(cats, keys, 1, 3));
	}

	@Test
	void anHourLongReplayPlansQuicklyAndSmall() {
		int n = 1_000_000;
		byte[] cats = new byte[n];
		long[] keys = new long[n];
		for (int i = 0; i < n; i++) {
			int kind = i % 10;
			int entity = (i / 10) % 200;
			int chunk = (i / 10) % 900;
			if (kind < 6) {
				cats[i] = Category.ENTITY_MOVE;
				keys[i] = entity;
			} else if (kind == 6) {
				cats[i] = Category.ENTITY_POS;
				keys[i] = entity;
			} else if (kind == 7) {
				cats[i] = Category.CHUNK;
				keys[i] = Category.chunk(chunk % 30, chunk / 30);
			} else if (kind == 8) {
				cats[i] = Category.BLOCK;
				keys[i] = Category.chunk(chunk % 30, chunk / 30);
			} else {
				cats[i] = Category.TRANSIENT;
			}
		}
		long start = System.nanoTime();
		int[] plan = SeekPlan.plan(cats, keys, 0, n);
		long ms = (System.nanoTime() - start) / 1_000_000;
		assertTrue(ms < 1000, "planning took " + ms + " ms");
		assertTrue(plan.length < n / 20, "plan kept " + plan.length + " of " + n);
	}

	@Test
	void longSetHandlesManyValuesAndTheSentinel() {
		SeekPlan.LongSet set = new SeekPlan.LongSet();
		for (long v = -5000; v < 5000; v++) {
			assertTrue(set.add(v * 31));
		}
		assertTrue(set.add(Long.MIN_VALUE));
		assertEquals(false, set.add(Long.MIN_VALUE));
		assertTrue(set.contains(31 * 4999L));
		assertEquals(false, set.contains(7));
	}
}
