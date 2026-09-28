package com.arcticlauncher.legacy.replay;

/** What the replay asks of the game's view, read by mixins; and this version's name. */
public final class LegacyReplayView {
	/** The field of view the replay camera asks for (0: the player's setting). */
	public static volatile float fov;

	/** "1.8.9", "1.12.2"… (the version a replay was recorded on). */
	//#if MC >= 1.12
	static final String VERSION = "1.12.2";
	//#elif MC >= 1.11
	static final String VERSION = "1.11.2";
	//#elif MC >= 1.10
	static final String VERSION = "1.10.2";
	//#elif MC >= 1.9
	static final String VERSION = "1.9.4";
	//#else
	static final String VERSION = "1.8.9";
	//#endif

	private LegacyReplayView() {}
}
