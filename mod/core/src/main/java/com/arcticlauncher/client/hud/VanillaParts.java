package com.arcticlauncher.client.hud;

/** The game's own HUD parts the HUD editor can move (see {@link VanillaPart}). */
public final class VanillaParts {
	public static final String SCOREBOARD = "scoreboard";
	public static final String BOSSBAR = "bossbar";
	/** The game's boss bar: 182 pixels wide, a bar every 19. */
	public static final int BOSSBAR_WIDTH = 182;
	public static final int BOSSBAR_STEP = 19;

	private VanillaParts() {}
}
