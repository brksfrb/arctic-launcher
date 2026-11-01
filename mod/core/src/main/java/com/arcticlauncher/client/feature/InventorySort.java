package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/**
 * A key in a chest or your inventory sorts the section under the mouse (the chest, or the 27 slots
 * of your inventory without the hotbar and armor): same kinds are merged, then ordered by name.
 * It works by clicking, exactly as you would: the clicks are worked out first on a copy, then sent
 * one per tick (a server sees a fast player, not a burst), and stop when the screen closes.
 */
public final class InventorySort {
	/** Quiet time after a sort so a held key doesn't start it again. */
	private static final long COOLDOWN_MS = 800;

	private static int[] slots;
	private static List<Integer> clicks;
	private static int next;
	private static long doneAt;

	private InventorySort() {}

	/** A kind of item in a slot; {@code count} changes while planning. */
	static final class Stack {
		final String key;
		final String name;
		int count;
		final int max;

		Stack(String key, String name, int count, int max) {
			this.key = key;
			this.name = name;
			this.count = count;
			this.max = Math.max(1, max);
		}
	}

	/** A key went down in a container screen (the adapter says so; this checks it is the sort key). */
	public static void pressed(ClientConfig c, Platform platform) {
		if (!c.inventorySort || running() || InventorySearch.active()
				|| System.currentTimeMillis() - doneAt < COOLDOWN_MS || !platform.isKeyDown(c.inventorySortKey)) {
			return;
		}
		int[] region = platform.sortSlots();
		if (region == null || region.length < 5 || platform.cursorHolds()) {
			return;
		}
		Stack[] state = new Stack[region.length];
		for (int i = 0; i < region.length; i++) {
			Object[] info = platform.slotInfo(region[i]);
			state[i] = info == null ? null : new Stack((String) info[0], (String) info[1], (Integer) info[2], (Integer) info[3]);
		}
		List<Integer> plan = plan(state);
		if (plan.isEmpty()) {
			return;
		}
		slots = region;
		clicks = plan;
		next = 0;
	}

	public static boolean running() {
		return clicks != null;
	}

	/** Every client tick: the next click of a sort in progress. */
	public static void tick(Platform platform) {
		if (clicks == null) {
			return;
		}
		if (!platform.sortScreenOpen()) {
			stop();
			return;
		}
		if (next < clicks.size()) {
			platform.slotClick(slots[clicks.get(next++)]);
			return;
		}
		// Whatever is left in the hand (something differed from the plan) goes into an empty slot.
		if (platform.cursorHolds()) {
			for (int slot : slots) {
				if (platform.slotInfo(slot) == null) {
					platform.slotClick(slot);
					break;
				}
			}
		}
		stop();
	}

	private static void stop() {
		clicks = null;
		slots = null;
		doneAt = System.currentTimeMillis();
	}

	/**
	 * The clicks (indexes into the section) that merge like stacks and put the kinds in name order,
	 * empty slots last. A click works as in the game: pick up, drop, merge or swap with the hand.
	 */
	static List<Integer> plan(Stack[] slots) {
		List<Integer> out = new ArrayList<Integer>();
		Stack[] s = Arrays.copyOf(slots, slots.length);
		Stack[] hand = new Stack[1];
		merge(s, hand, out);
		order(s, hand, out);
		return out;
	}

	/** One click on slot {@code i}, applied to the copy. */
	private static void click(Stack[] s, Stack[] hand, int i, List<Integer> out) {
		out.add(i);
		if (hand[0] == null) {
			hand[0] = s[i];
			s[i] = null;
		} else if (s[i] == null) {
			s[i] = hand[0];
			hand[0] = null;
		} else if (s[i].key.equals(hand[0].key)) {
			int moved = Math.min(s[i].max - s[i].count, hand[0].count);
			s[i].count += moved;
			hand[0].count -= moved;
			if (hand[0].count == 0) {
				hand[0] = null;
			}
		} else {
			Stack held = hand[0];
			hand[0] = s[i];
			s[i] = held;
		}
	}

	/** Fill each stack that has room from the later stacks of its kind. */
	private static void merge(Stack[] s, Stack[] hand, List<Integer> out) {
		for (int a = 0; a < s.length; a++) {
			for (int b = s.length - 1; b > a && s[a] != null && s[a].count < s[a].max; b--) {
				if (s[b] != null && s[b].key.equals(s[a].key)) {
					click(s, hand, b, out);
					click(s, hand, a, out);
					if (hand[0] != null) {
						click(s, hand, b, out);
					}
				}
			}
		}
	}

	/** Selection sort by kind: the wanted kind for each position is swapped in. */
	private static void order(Stack[] s, Stack[] hand, List<Integer> out) {
		List<Stack> wanted = new ArrayList<Stack>();
		for (Stack st : s) {
			if (st != null) {
				wanted.add(st);
			}
		}
		sortByName(wanted);
		for (int i = 0; i < s.length; i++) {
			Stack want = i < wanted.size() ? wanted.get(i) : null;
			if (want == null ? s[i] == null : s[i] != null && s[i].key.equals(want.key)) {
				continue;
			}
			int from = -1;
			for (int j = i + 1; j < s.length && want != null; j++) {
				if (s[j] != null && s[j].key.equals(want.key) && (from < 0 || s[j].count > s[from].count)) {
					from = j;
				}
			}
			if (from < 0) {
				continue;
			}
			click(s, hand, from, out);
			click(s, hand, i, out);
			if (hand[0] != null) {
				click(s, hand, from, out);
			}
		}
	}

	/** Name order, then bigger stacks first. */
	private static void sortByName(List<Stack> stacks) {
		java.util.Collections.sort(stacks, new Comparator<Stack>() {
			@Override
			public int compare(Stack x, Stack y) {
				int byName = x.name.compareToIgnoreCase(y.name);
				if (byName != 0) {
					return byName;
				}
				int byKey = x.key.compareTo(y.key);
				return byKey != 0 ? byKey : Integer.compare(y.count, x.count);
			}
		});
	}
}
