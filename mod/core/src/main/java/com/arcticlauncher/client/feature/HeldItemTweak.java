package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Held Item: moves and resizes your hands and the item in them in first person (what PvP clients call
 * "item positions"). Offsets are in hundredths of a block in your view: x right, y up, z away.
 */
public final class HeldItemTweak {
	private HeldItemTweak() {}

	/** {x, y, z, scale} to apply, or null when it is off (or all at the game's own values). */
	public static float[] values(ClientConfig c) {
		if (c == null || !c.heldItem) {
			return null;
		}
		if (c.heldX == 0 && c.heldY == 0 && c.heldZ == 0 && c.heldScale == 100) {
			return null;
		}
		return new float[] {c.heldX / 100f, c.heldY / 100f, c.heldZ / 100f, Math.max(10, c.heldScale) / 100f};
	}
}
