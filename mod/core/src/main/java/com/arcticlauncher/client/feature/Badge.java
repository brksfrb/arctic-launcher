package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.UUID;

/** Where the Arctic badge (the snowflake) shows, and for whom. */
public final class Badge {
	/** {@link ClientConfig#badgeWhere}: the player list and name tags. */
	public static final int BOTH = 0;
	public static final int TAB_ONLY = 1;
	public static final int TAGS_ONLY = 2;
	public static final String[] WHERE = {"Player list and name tags", "Player list only", "Name tags only"};

	private Badge() {}

	/** Whether {@code player} gets the badge in the player list ({@code tab}) or on their name tag. */
	public static boolean show(boolean tab, UUID player) {
		ClientConfig config = ArcticClient.config();
		if (config == null || player == null || ArcticClient.looks() == null) {
			return false;
		}
		if (tab ? config.badgeWhere == TAGS_ONLY : !config.badgeNametags || config.badgeWhere == TAB_ONLY) {
			return false;
		}
		if (config.badgeOthersOnly) {
			Platform platform = ArcticClient.platform();
			if (platform != null && player.equals(platform.worldPlayerId())) {
				return false;
			}
		}
		return ArcticClient.looks().isArctic(player);
	}
}
