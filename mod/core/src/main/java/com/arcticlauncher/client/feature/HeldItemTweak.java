package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Held Item: moves and resizes your hands and the item in them in first person (what PvP clients call
 * "item positions"). Offsets are in hundredths of a block in your view: x right, y up, z away.
 */
public final class HeldItemTweak {
	private HeldItemTweak() {}

	/**
	 * Where the hands sit in view space (right, up, away). Scaling around the camera would change nothing
	 * on screen, so the size is applied around this point instead.
	 */
	private static final float PIVOT_X = 0.4f;
	private static final float PIVOT_Y = -0.5f;
	private static final float PIVOT_Z = -0.7f;

	/**
	 * {x, y, z, scale} to apply as translate then scale, or null when it is off (or all at the game's own
	 * values). The translation includes the shift that keeps the hands' pivot in place while they scale.
	 */
	public static float[] values(ClientConfig c) {
		if (c == null || !c.heldItem) {
			return null;
		}
		if (c.heldX == 0 && c.heldY == 0 && c.heldZ == 0 && c.heldScale == 100) {
			return null;
		}
		float scale = Math.max(10, c.heldScale) / 100f;
		float keep = 1f - scale;
		return new float[] {c.heldX / 100f + PIVOT_X * keep, c.heldY / 100f + PIVOT_Y * keep,
				c.heldZ / 100f + PIVOT_Z * keep, scale};
	}
}
