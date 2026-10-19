package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import java.util.UUID;

/** "48ms | " in front of other players' name tags (the same place PvP tier tags go). */
public final class PingTags {
	private PingTags() {}

	/** The text to put in front of a player's name, or null (switched off, unknown, or yourself). */
	public static String prefix(UUID player) {
		if (!ArcticClient.config().pingNametags) {
			return null;
		}
		Platform platform = ArcticClient.platform();
		if (platform == null) {
			return null;
		}
		return format(platform.ping(player));
	}

	/** "48ms | " in its colour, or null when the ping isn't known. */
	public static String format(int ms) {
		return ms <= 0 ? null : "§" + colour(ms) + ms + "ms §8| §r";
	}

	/** Green when it's good, red when it isn't: the chat colour code. */
	static char colour(int ms) {
		if (ms < 80) {
			return 'a';
		}
		if (ms < 150) {
			return 'e';
		}
		return ms < 250 ? '6' : 'c';
	}
}
