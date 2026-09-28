package com.arcticlauncher.client.replay;

import java.util.Arrays;

/**
 * Which packets to apply to get from one moment of a replay to a later one
 * as fast as possible: every packet whose effect a later one replaces is
 * left out (older copies of a chunk, block changes inside it, moves before
 * a teleport, anything before a respawn). An hour-long replay jumps in the
 * time it takes to load the chunks you'll actually see.
 */
public final class SeekPlan {
	private SeekPlan() {}

	/**
	 * Packets in {@code [from, to)} worth applying, in order. {@code category}
	 * and {@code key} are {@link ReplayData}'s classification.
	 */
	public static int[] plan(byte[] category, long[] key, int from, int to) {
		int[] out = new int[Math.max(16, (to - from) / 8)];
		int n = 0;
		LongSet chunksLater = new LongSet();
		LongSet placedLater = new LongSet();
		boolean respawnLater = false;
		for (int i = to - 1; i >= from; i--) {
			byte c = category[i];
			long k = key[i];
			boolean keep;
			switch (c) {
				case Category.ALWAYS:
					keep = true;
					break;
				case Category.RESPAWN:
					// Only the last one matters: each starts a new world.
					keep = !respawnLater;
					respawnLater = true;
					break;
				case Category.CHUNK:
				case Category.UNLOAD:
					keep = !respawnLater && chunksLater.add(k);
					break;
				case Category.BLOCK:
					keep = !respawnLater && !chunksLater.contains(k);
					break;
				case Category.ENTITY_ADD:
					keep = !respawnLater;
					// Moves before it were for an earlier entity with this id.
					placedLater.add(k);
					break;
				case Category.ENTITY_POS:
					keep = !respawnLater && placedLater.add(k);
					break;
				case Category.ENTITY_MOVE:
					keep = !respawnLater && !placedLater.contains(k);
					break;
				case Category.ENTITY_OTHER:
					keep = !respawnLater;
					break;
				default:
					// TRANSIENT and DROP: nothing to see when jumping.
					keep = false;
					break;
			}
			if (keep) {
				if (n == out.length) {
					out = Arrays.copyOf(out, out.length * 2);
				}
				out[n++] = i;
			}
		}
		int[] ordered = new int[n];
		for (int i = 0; i < n; i++) {
			ordered[i] = out[n - 1 - i];
		}
		return ordered;
	}

	/** An open-addressing set of longs (no boxing: plans scan a million packets). */
	static final class LongSet {
		private static final long EMPTY = Long.MIN_VALUE;
		private long[] slots = new long[1024];
		private int size;
		private boolean hasEmpty;

		LongSet() {
			Arrays.fill(slots, EMPTY);
		}

		/** Add; true if it wasn't there. */
		boolean add(long v) {
			if (v == EMPTY) {
				boolean was = hasEmpty;
				hasEmpty = true;
				return !was;
			}
			if ((size + 1) * 2 > slots.length) {
				grow();
			}
			int mask = slots.length - 1;
			int i = hash(v) & mask;
			while (slots[i] != EMPTY) {
				if (slots[i] == v) {
					return false;
				}
				i = (i + 1) & mask;
			}
			slots[i] = v;
			size++;
			return true;
		}

		boolean contains(long v) {
			if (v == EMPTY) {
				return hasEmpty;
			}
			int mask = slots.length - 1;
			int i = hash(v) & mask;
			while (slots[i] != EMPTY) {
				if (slots[i] == v) {
					return true;
				}
				i = (i + 1) & mask;
			}
			return false;
		}

		private void grow() {
			long[] old = slots;
			slots = new long[old.length * 2];
			Arrays.fill(slots, EMPTY);
			size = 0;
			for (long v : old) {
				if (v != EMPTY) {
					add(v);
				}
			}
		}

		private static int hash(long v) {
			long h = v * 0x9E3779B97F4A7C15L;
			return (int) (h ^ (h >>> 32));
		}
	}
}
