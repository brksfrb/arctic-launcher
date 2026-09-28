package com.arcticlauncher.mod.replay;

/** What the replay asks of the game's view (read by mixins on every version). */
public final class ReplayView {
	/** The field of view the replay camera asks for (0: the player's setting). */
	public static volatile float fov;

	private ReplayView() {}
}
