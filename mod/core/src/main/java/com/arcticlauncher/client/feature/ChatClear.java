package com.arcticlauncher.client.feature;

/**
 * The "Clear chat" key: the game's own clear is skipped while "keep chat when leaving" is on, so the
 * key raises this flag around its call and the chat hook lets that one clear through.
 */
public final class ChatClear {
	private static boolean forced;

	private ChatClear() {}

	/** Is a clear the player asked for running right now? Game thread. */
	public static boolean forced() {
		return forced;
	}

	/** Run {@code clear} (the game's chat clear) as the player's request. */
	public static void run(Runnable clear) {
		forced = true;
		try {
			clear.run();
		} finally {
			forced = false;
		}
	}
}
