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
	public static final String[] WHERE = {"mods.badge_where.player_list_and_name_tags", "mods.badge_where.player_list_only", "mods.badge_where.name_tags_only"};

	/** A shipped style. IDs are the stable looks API values, never list positions; the label is a text key. */
	public static final class Style {
		public final int id;
		public final String label;
		public final String texture;
		public final String font;
		public final boolean roleRecolor;

		private Style(int id, String label, String texture, String font, boolean roleRecolor) {
			this.id = id;
			this.label = label;
			this.texture = texture;
			this.font = font;
			this.roleRecolor = roleRecolor;
		}
	}

	public static final java.util.List<Style> STYLES = java.util.Collections.unmodifiableList(java.util.Arrays.asList(
			new Style(0, "hud.arctic_logo", "badge", "badge", true),
			new Style(1, "hud.ice_tile", "badge_ice_tile", "badge_ice_tile", true),
			new Style(2, "hud.line_tile", "badge_line_tile", "badge_line_tile", true)));
	/** Wire roles are allowlisted; never use server text directly as an asset path. */
	public static final String[] ROLES = {"", "founder", "dev", "admin", "mod", "beta"};
	public static final int[] COLORS = {0x2EB1EC, 0xD99A0B, 0xA8B4C0, 0xFF5A5A, 0x2563EB, 0xB07CFF};
	public static int roleIndex(String role) {
		for (int i = 1; i < ROLES.length; i++) if (ROLES[i].equals(role)) return i;
		return 0;
	}
	public static String role(UUID wearer) {
		return ArcticClient.looks() == null ? null : ArcticClient.looks().badgeRole(wearer);
	}
	public static final int MAX_RASTER_SIZE = 128;
	private static final int ROLE_GLYPH_BASE = 0xE300;
	private static final int ROLE_STRIDE = 8;

	public static char glyph(int style, int role, boolean tab) {
		if (role == 0) return (char) (GLYPH_BASE + normalize(style) + (tab ? 16 : 0));
		return (char) (ROLE_GLYPH_BASE + (role - 1) * 16 + normalize(style) + (tab ? ROLE_STRIDE : 0));
	}

	public static int glyphSlot(int codepoint) {
		int role = 0;
		int style = codepoint - GLYPH_BASE;
		boolean tab = style >= 16;
		if (codepoint >= ROLE_GLYPH_BASE) {
			int code = codepoint - ROLE_GLYPH_BASE;
			role = 1 + code / 16;
			tab = code % 16 >= ROLE_STRIDE;
			style = code % ROLE_STRIDE;
		} else if (tab) {
			style -= 16;
		}
		return style >= 0 && style < STYLES.size() && role < ROLES.length ? slot(style, role, tab) : -1;
	}

	public static int slot(int style, int role, boolean tab) {
		return (style * ROLES.length + role) * 2 + (tab ? 1 : 0);
	}

	public static final int SIZE = 8;
	public static final int TAB_SIZE = 9;
	public static final int TEXTURE_SIZE = 256;
	/** Bitmap fallback characters for versions before text styles could select fonts. */
	public static final char GLYPH_BASE = '\uE0A0';
	public static final char GLYPH_GAP = '\uE0AF';
	private static final int TAB_OFFSET = 0x100;
	private static final char MARK_BASE = '\uE100';

	public static Style definition(int id) {
		for (Style style : STYLES) {
			if (style.id == id) return style;
		}
		return STYLES.get(0);
	}

	public static int normalize(int id) {
		return definition(id).id;
	}

	/** The wearer's published style, independent of this viewer's settings or cosmetic previews. */
	public static int style(UUID wearer) {
		return ArcticClient.looks() == null ? 0 : ArcticClient.looks().badgeStyle(wearer);
	}

	public static String name(int style) {
		return definition(style).texture;
	}

	public static int size(boolean tab) {
		return tab ? TAB_SIZE : SIZE;
	}

	public static int advance(boolean tab) {
		return size(tab) + 2;
	}

	/** Bound the texture cache while retaining integer framebuffer sizes at ordinary GUI scales. */
	/**
	 * Framebuffer pixels of one badge, quantized to a whole pixel from the projected size
	 * ({@code size(tab) * pixelsPerUnit}). {@code pixelsPerUnit} is {@link TextPixelScale#projected}:
	 * camera distance, FOV, Arctic zoom, sprint FOV and GUI scale are already in that number.
	 */
	public static int rasterSize(boolean tab, float pixelsPerUnit) {
		return Math.max(1, Math.min(MAX_RASTER_SIZE, Math.round(size(tab) * pixelsPerUnit)));
	}

	/** Text-space side that covers exactly {@code resolution} framebuffer pixels at this scale. */
	public static float quadSide(int resolution, float pixelsPerUnit) {
		return pixelsPerUnit > 0f ? resolution / pixelsPerUnit : resolution;
	}

	/** The badge picture never has a drop shadow; the name text keeps its own. */
	public static boolean drawShadow(boolean tab, boolean textShadow) {
		return false;
	}

	/** Carry wearer and placement through old String text APIs; never consult a local style while drawing. */
	public static String mark(boolean tab, UUID wearer) {
		return mark(tab, style(wearer), role(wearer));
	}

	public static String mark(boolean tab, int style, String role) {
		return "\u00a7r" + (char) (MARK_BASE + normalize(style) + roleIndex(role) * ROLE_STRIDE + (tab ? TAB_OFFSET : 0)) + "\u00a7r";
	}

	public static boolean marked(String text) {
		if (text == null || text.length() < 5 || !text.startsWith("\u00a7r") || !text.substring(3, 5).equals("\u00a7r")) return false;
		int code = text.charAt(2) - MARK_BASE;
		return code >= 0 && code < 128 || code >= TAB_OFFSET && code < TAB_OFFSET + 128;
	}

	public static boolean markedTab(String text) {
		return text.charAt(2) - MARK_BASE >= TAB_OFFSET;
	}

	public static int markedStyle(String text) {
		int code = (text.charAt(2) - MARK_BASE) % TAB_OFFSET;
		return code / ROLE_STRIDE < ROLES.length ? normalize(code % ROLE_STRIDE) : 0;
	}

	public static String markedRole(String text) {
		int role = (text.charAt(2) - MARK_BASE) % TAB_OFFSET / ROLE_STRIDE;
		return role < ROLES.length ? ROLES[role] : "";
	}

	public static String unmark(String text) {
		while (marked(text)) text = text.substring(5);
		return text;
	}

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
