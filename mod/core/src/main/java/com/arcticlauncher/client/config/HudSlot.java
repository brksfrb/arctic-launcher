package com.arcticlauncher.client.config;

/**
 * Where a HUD widget sits. Widgets the player hasn't moved stack neatly on
 * their own; once moved, a widget is pinned to the nearest screen edge
 * ({@link #ax}, {@link #ay}) at an offset, so it stays put across window
 * sizes and GUI scales.
 */
public final class HudSlot {
	public static final int START = 0;
	public static final int CENTER = 1;
	public static final int END = 2;

	public boolean enabled;
	/** Moved by the player (otherwise laid out automatically). */
	public boolean placed;
	/** Horizontal anchor: {@link #START} (left), {@link #CENTER} or {@link #END} (right). */
	public int ax;
	/** Vertical anchor: {@link #START} (top), {@link #CENTER} or {@link #END} (bottom). */
	public int ay;
	/** Offset from the anchor, in GUI pixels, toward the middle of the screen. */
	public int dx;
	public int dy;
	public float scale = 1f;

	public HudSlot() {}

	public HudSlot(boolean enabled) {
		this.enabled = enabled;
	}

	public HudSlot copy() {
		HudSlot c = new HudSlot(enabled);
		c.placed = placed;
		c.ax = ax;
		c.ay = ay;
		c.dx = dx;
		c.dy = dy;
		c.scale = scale;
		return c;
	}
}
