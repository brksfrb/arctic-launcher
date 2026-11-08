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

	/** {@link ClientConfig#badgeStyle}: the pictures (assets/arctic/badge*.png, font/badge*.json), local display only. */
	public static final String[] STYLES = {"Snowflake", "Snowflake tile", "Round", "Letter A"};
	/** The picture's side in GUI pixels, drawn from the top of the text line (the font's ascent is 7, its glyphs 8 high). */
	public static final int SIZE = 8;
	/** From the badge's left edge to the name: the picture, 1px of its own, and a 1px gap. Same on every version. */
	public static final int ADVANCE = 10;
	/** First private-use character of the badge pictures in the default font (1.13-1.15, where text styles have no font). */
	public static final char GLYPH_BASE = '\uE0A0';
	/** A 1px space in that font. */
	public static final char GLYPH_GAP = '\uE0AF';

	/**
	 * The blank text 1.8.9-1.12.2 (no fonts in text) put in front of a name: the text renderer draws the
	 * picture over it and measures it as {@link #ADVANCE} wide whatever the font (three spaces if a
	 * draw path is not hooked).
	 */
	public static final String MARK = "§r   ";

	/** The chosen style, in range. */
	public static int style() {
		ClientConfig config = ArcticClient.config();
		int style = config == null ? 0 : config.badgeStyle;
		return style < 0 || style >= STYLES.length ? 0 : style;
	}

	/** The picture's file name without the folder: badge, badge1, ... */
	public static String name(int style) {
		return style <= 0 ? "badge" : "badge" + style;
	}

	/** {@code -Darctic.selftest=badge}: every player has the badge, so a test world can show it. */
	private static final boolean TEST = "badge".equals(System.getProperty("arctic.selftest"));

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
		if (TEST) {
			return true;
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
