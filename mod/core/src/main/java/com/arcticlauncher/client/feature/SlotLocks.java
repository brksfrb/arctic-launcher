package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.Arrays;

/**
 * Locked inventory slots: the item in a locked slot can't be moved, swapped, shift-clicked or dropped
 * by accident. Hold the lock key and click a slot to lock or unlock it. Slots are the player's own
 * inventory numbered as the game does: 0-8 hotbar, 9-35 the rest, 36-39 armor (boots first), 40 off hand.
 */
public final class SlotLocks {
	/** Highest slot number of the player's inventory. */
	public static final int LAST_SLOT = 40;

	private SlotLocks() {}

	public static boolean locked(ClientConfig c, int slot) {
		if (c.lockedSlots == null) {
			return false;
		}
		for (int s : c.lockedSlots) {
			if (s == slot) {
				return true;
			}
		}
		return false;
	}

	/** Locks the slot, or unlocks it when it was locked. */
	public static void toggle(ClientConfig c, int slot) {
		if (slot < 0 || slot > LAST_SLOT) {
			return;
		}
		int[] now = c.lockedSlots == null ? new int[0] : c.lockedSlots;
		if (locked(c, slot)) {
			int[] left = new int[now.length - 1];
			int n = 0;
			for (int s : now) {
				if (s != slot) {
					left[n++] = s;
				}
			}
			c.lockedSlots = left;
		} else {
			int[] more = Arrays.copyOf(now, now.length + 1);
			more[now.length] = slot;
			c.lockedSlots = more;
		}
	}

	/**
	 * A click in an inventory screen. {@code slot}: the clicked slot's number in the player's inventory,
	 * or -1 when it is in another container (a chest). {@code swapWith}: for a hotbar-key or off-hand
	 * swap, the inventory slot it swaps with (0-8 or 40), else -1. True to drop the click.
	 */
	public static boolean click(ClientConfig c, Platform platform, int slot, int swapWith) {
		if (!c.slotLocks) {
			return false;
		}
		if (slot >= 0 && platform.isKeyDown(c.slotLockKey)) {
			toggle(c, slot);
			ArcticClient.saveConfig();
			return true;
		}
		return slot >= 0 && locked(c, slot) || swapWith >= 0 && locked(c, swapWith);
	}

	/** A padlock on a locked slot whose top-left corner is at ({@code x}, {@code y}), 16 pixels wide. */
	public static void draw(com.arcticlauncher.client.gfx.Gfx g, int x, int y) {
		g.fill(x, y, x + 16, y + 16, 0x33FFD84D);
		// Shackle, body and keyhole of a small padlock in the corner.
		g.fill(x + 10, y + 1, x + 14, y + 3, 0xFF3A2E00);
		g.fill(x + 11, y + 2, x + 13, y + 5, 0xFF3A2E00);
		g.fill(x + 9, y + 4, x + 15, y + 9, 0xFF3A2E00);
		g.fill(x + 10, y + 5, x + 14, y + 8, 0xFFFFD84D);
		g.fill(x + 11, y + 2, x + 13, y + 4, 0xFFFFD84D);
		g.fill(x + 11, y + 2, x + 12, y + 3, 0xFFFFD84D);
		g.fill(x + 11, y + 6, x + 13, y + 7, 0xFF3A2E00);
	}

	/** The drop key: true to drop the press, when the slot in hand is locked. */
	public static boolean drop(ClientConfig c, int selectedHotbarSlot) {
		return c.slotLocks && locked(c, selectedHotbarSlot);
	}
}
