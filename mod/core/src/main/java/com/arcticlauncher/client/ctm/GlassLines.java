package com.arcticlauncher.client.ctm;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * The "Clear glass lines" level and when to apply it. The level is baked into the glass tiles when the block
 * atlas is stitched, so a change needs the textures loaded again; that waits until the slider has rested.
 */
public final class GlassLines {
	/** Client ticks a new level must stay unchanged before the textures are loaded again. */
	static final int REST = 12;
	private static volatile int stitched = -1;
	private static int pending = -1, rested;
	private GlassLines() {}

	public static int level() {
		ClientConfig config = ArcticClient.config();
		return config == null ? ClientConfig.CLEAR_GLASS_LINES : Math.max(0, Math.min(100, config.clearGlassLines));
	}

	/** The level the tiles are generated with. */
	public static int stitched() {
		int now = stitched;
		return now < 0 ? stitched = level() : now;
	}

	/** Once per client tick: true when a changed level has rested and the textures should be loaded again. */
	public static boolean due() { return due(level()); }

	/** As {@link #due()}, but with Connected Glass off nothing shows the level, so the textures are left alone until it is on again. */
	public static boolean due(boolean enabled) { return due(level(), enabled); }

	static boolean due(int level, boolean enabled) {
		if (enabled) return due(level);
		pending = -1;
		return false;
	}

	static boolean due(int level) {
		if (stitched < 0) stitched = level;
		if (level == stitched) { pending = -1; return false; }
		if (level != pending) { pending = level; rested = 0; return false; }
		if (++rested < REST) return false;
		pending = -1;
		stitched = level;
		return true;
	}

	static void reset() { stitched = -1; pending = -1; rested = 0; }
}
