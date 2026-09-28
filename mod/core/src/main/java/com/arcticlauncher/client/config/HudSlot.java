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
	/** Draw the widget's backdrop panel. */
	public boolean background = true;

	// ---- How it looks (Lunar-like by default: flat black box, white shadowed text) ----

	/** Label formats: "FPS: 144", "144 FPS", just "144". */
	public static final int LABEL_BEFORE = 0;
	public static final int LABEL_AFTER = 1;
	public static final int LABEL_NONE = 2;

	public int textColor = 0xFFFFFFFF;
	/** The label's own color; 0 = the text color. */
	public int labelColor;
	public int backgroundColor = 0x6F000000;
	public boolean border;
	public int borderColor = 0xA0FFFFFF;
	public boolean shadow = true;
	/** Text cycles through the rainbow. */
	public boolean chroma;
	/** "[FPS: 144]". */
	public boolean brackets;
	public int labelMode = LABEL_BEFORE;
	/** Rounded box corners (0 = square). */
	public int radius;

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
		c.background = background;
		c.textColor = textColor;
		c.labelColor = labelColor;
		c.backgroundColor = backgroundColor;
		c.border = border;
		c.borderColor = borderColor;
		c.shadow = shadow;
		c.chroma = chroma;
		c.brackets = brackets;
		c.labelMode = labelMode;
		c.radius = radius;
		return c;
	}
}
