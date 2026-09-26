package com.arcticlauncher.client.config;

/** A custom crosshair (drawn instead of Minecraft's when enabled). */
public final class CrosshairConfig {
	public static final String[] STYLES = {"cross", "dot", "circle", "cross-dot"};
	public static final int[] COLORS = {0xFFFFFFFF, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFF000000};

	public boolean enabled;
	public String style = "cross";
	/** Arm length in GUI pixels. */
	public int size = 5;
	/** Empty space in the middle. */
	public int gap = 2;
	public int thickness = 1;
	public int color = 0xFFFFFFFF;
	/** A dark outline so it shows on bright blocks. */
	public boolean outline = true;
}
