package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import com.arcticlauncher.client.feature.InventorySort.Stack;
import java.util.List;
import org.junit.jupiter.api.Test;

class InventorySortTest {
	private static Stack stack(String name, int count, int max) {
		return new Stack(name, name, count, max);
	}

	/** Plays the clicks on a copy the way the game does, to see where the items end up. */
	private static Stack[] play(Stack[] start, List<Integer> clicks) {
		Stack[] s = new Stack[start.length];
		for (int i = 0; i < s.length; i++) {
			s[i] = start[i] == null ? null : stack(start[i].key, start[i].count, start[i].max);
		}
		Stack hand = null;
		for (int i : clicks) {
			if (hand == null) {
				hand = s[i];
				s[i] = null;
			} else if (s[i] == null) {
				s[i] = hand;
				hand = null;
			} else if (s[i].key.equals(hand.key)) {
				int moved = Math.min(s[i].max - s[i].count, hand.count);
				s[i].count += moved;
				hand.count -= moved;
				hand = hand.count == 0 ? null : hand;
			} else {
				Stack t = hand;
				hand = s[i];
				s[i] = t;
			}
		}
		assertNull(hand, "nothing is left in the hand");
		return s;
	}

	@Test
	void mergesLikeStacksAndOrdersByName() {
		Stack[] start = {stack("Stone", 40, 64), null, stack("Apple", 10, 64), stack("Stone", 40, 64), stack("Dirt", 5, 64), null};
		Stack[] end = play(start, InventorySort.plan(start));
		assertEquals("Apple", end[0].key);
		assertEquals("Dirt", end[1].key);
		assertEquals("Stone", end[2].key);
		assertEquals(64, end[2].count);
		assertEquals("Stone", end[3].key);
		assertEquals(16, end[3].count);
		assertNull(end[4]);
		assertNull(end[5]);
	}

	@Test
	void anAlreadySortedSectionNeedsNoClicks() {
		Stack[] start = {stack("Apple", 1, 64), stack("Dirt", 2, 64), null};
		assertEquals(0, InventorySort.plan(start).size());
	}

	@Test
	void unstackableItemsMoveWithoutMerging() {
		Stack[] start = {stack("Sword", 1, 1), stack("Axe", 1, 1), stack("Sword", 1, 1)};
		Stack[] end = play(start, InventorySort.plan(start));
		assertEquals("Axe", end[0].key);
		assertEquals("Sword", end[1].key);
		assertEquals("Sword", end[2].key);
	}
}
