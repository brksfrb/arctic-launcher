package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.gfx.Gfx;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** Approved proposal 2. The private glyphs exist only in chat's layout copy, never on the wire. */
public final class EmoteAtlas {
	public static final int MAX_CODE = 32;
	public static final int FIRST_GLYPH = 0xE100;
	public static final int ADVANCE = 10;
	public static final int CELL = 144;
	public static final int SIZE = 1152;
	private static final List<Emote> EMOTES = new ArrayList<Emote>();
	private static final Map<String, Emote> NAMES = new LinkedHashMap<String, Emote>();

	public static final class Emote {
		public final String group;
		public final String code;
		public final int index;
		private Emote(String group, String code, int index) {
			this.group = group;
			this.code = code;
			this.index = index;
		}
		public String shortcode() { return ":" + code + ":"; }
		public char glyph() { return (char) (FIRST_GLYPH + index); }
		public int u() { return index % 8 * CELL; }
		public int v() { return index / 8 * CELL; }
		public void draw(Gfx g, int x, int y, int size) {
			g.texture("asset:emotes", x, y, size, size, u(), v(), CELL, CELL, SIZE, SIZE);
		}
	}

	private static void add(String group, String code, String... aliases) {
		Emote emote = new Emote(group, code, EMOTES.size());
		EMOTES.add(emote);
		NAMES.put(code, emote);
		for (String alias : aliases) {
			if (NAMES.put(alias, emote) != null) throw new IllegalStateException(alias);
		}
	}
	static {
		// BEGIN GENERATED
		add("Faces", "slight_smile", "slight");
		add("Faces", "smile", "grinning");
		add("Faces", "joy", "laugh");
		add("Faces", "wink");
		add("Faces", "blush");
		add("Faces", "sob");
		add("Faces", "cry");
		add("Faces", "sad", "disappointed");
		add("Faces", "wow", "open_mouth");
		add("Faces", "thinking", "think");
		add("Faces", "cool", "sunglasses");
		add("Faces", "angry");
		add("Faces", "sweat_smile");
		add("Faces", "rofl");
		add("Faces", "heart_eyes");
		add("Faces", "kissing_heart", "kiss");
		add("Faces", "smirk");
		add("Faces", "tongue", "stuck_out_tongue_winking");
		add("Faces", "flushed");
		add("Faces", "scream");
		add("Faces", "pleading");
		add("Faces", "rolling_eyes");
		add("Faces", "sleeping");
		add("Faces", "partying", "party");
		add("Faces", "clown");
		add("Faces", "exploding_head");
		add("Faces", "smiling_imp", "devil");
		add("Faces", "hug", "hugging");
		add("Reactions and symbols", "skull");
		add("Reactions and symbols", "heart");
		add("Reactions and symbols", "blue_heart");
		add("Reactions and symbols", "fire");
		add("Reactions and symbols", "thumbsup", "+1");
		add("Reactions and symbols", "thumbsdown", "-1");
		add("Reactions and symbols", "clap");
		add("Reactions and symbols", "wave");
		add("Reactions and symbols", "eyes");
		add("Reactions and symbols", "pray");
		add("Reactions and symbols", "check", "white_check_mark");
		add("Reactions and symbols", "x", "cross");
		add("Reactions and symbols", "star");
		add("Reactions and symbols", "sparkles");
		add("Reactions and symbols", "zzz");
		add("Reactions and symbols", "crown");
		add("Reactions and symbols", "ok_hand");
		add("Reactions and symbols", "muscle");
		add("Reactions and symbols", "raised_hands");
		add("Reactions and symbols", "broken_heart");
		add("Reactions and symbols", "100");
		add("Reactions and symbols", "tada");
		add("Reactions and symbols", "trophy");
		add("Reactions and symbols", "crossed_swords", "swords");
		add("Reactions and symbols", "gem");
		add("Reactions and symbols", "boom");
		add("Reactions and symbols", "ghost");
		add("Reactions and symbols", "pick", "pickaxe");
		add("Arctic", "arctic");
		add("Arctic", "snowflake", "snow");
		add("Arctic", "ice");
		add("Arctic", "cold", "cold_face");
		add("Arctic", "penguin");
		add("Arctic", "snowman");
		add("Arctic", "ice_cube", "iceberg");
		add("Arctic", "bear");
		// END GENERATED
	}
	private EmoteAtlas() {}
	public static List<Emote> all() { return Collections.unmodifiableList(EMOTES); }
	public static Emote named(String name) { return name == null ? null : NAMES.get(name.toLowerCase(Locale.ROOT)); }
	public static Emote glyph(int cp) {
		int index = cp - FIRST_GLYPH;
		return index >= 0 && index < EMOTES.size() ? EMOTES.get(index) : null;
	}
	/** Match only a complete pair of colons, with a bounded name. */
	public static Emote at(String text, int start) {
		if (text.charAt(start) != ':') return null;
		int end = text.indexOf(':', start + 1);
		return end > start + 1 && end - start - 1 <= MAX_CODE ? named(text.substring(start + 1, end)) : null;
	}
	public static String layout(String text, boolean enabled) {
		if (!enabled || text == null || text.indexOf(':') < 0) return text;
		StringBuilder out = new StringBuilder(text.length());
		for (int i = 0; i < text.length(); i++) {
			Emote emote = at(text, i);
			if (emote == null) out.append(text.charAt(i));
			else {
				out.append(emote.glyph());
				i = text.indexOf(':', i + 1);
			}
		}
		return out.toString();
	}
}
