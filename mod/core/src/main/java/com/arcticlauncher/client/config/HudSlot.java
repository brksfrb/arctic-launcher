package com.arcticlauncher.client.config;

/**
 * Where a HUD widget sits. {@code x} and {@code y} are fractions of the free
 * space (0 = left/top edge, 1 = right/bottom edge), so layouts survive
 * window resizes and GUI scale changes.
 */
public final class HudSlot {
	public boolean enabled;
	public float x;
	public float y;
	public float scale = 1f;

	public HudSlot() {}

	public HudSlot(boolean enabled, float x, float y) {
		this.enabled = enabled;
		this.x = x;
		this.y = y;
	}

	public HudSlot copy() {
		HudSlot c = new HudSlot(enabled, x, y);
		c.scale = scale;
		return c;
	}
}
