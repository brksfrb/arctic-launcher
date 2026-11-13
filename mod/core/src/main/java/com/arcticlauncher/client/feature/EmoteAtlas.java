package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.gfx.Gfx;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

import com.arcticlauncher.client.Lang;
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
		/** The group's text key (shown through {@link Lang#t}). */
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
		add("hud.faces", "slight_smile", "slight");
		add("hud.faces", "smile", "grinning");
		add("hud.faces", "joy", "laugh");
		add("hud.faces", "wink");
		add("hud.faces", "blush");
		add("hud.faces", "sob");
		add("hud.faces", "cry");
		add("hud.faces", "sad", "disappointed");
		add("hud.faces", "wow", "open_mouth");
		add("hud.faces", "thinking", "think");
		add("hud.faces", "cool", "sunglasses");
		add("hud.faces", "angry");
		add("hud.faces", "sweat_smile");
		add("hud.faces", "rofl");
		add("hud.faces", "heart_eyes");
		add("hud.faces", "kissing_heart", "kiss");
		add("hud.faces", "smirk");
		add("hud.faces", "tongue", "stuck_out_tongue_winking");
		add("hud.faces", "flushed");
		add("hud.faces", "scream");
		add("hud.faces", "pleading");
		add("hud.faces", "rolling_eyes");
		add("hud.faces", "sleeping");
		add("hud.faces", "partying", "party");
		add("hud.faces", "clown");
		add("hud.faces", "exploding_head");
		add("hud.faces", "smiling_imp", "devil");
		add("hud.faces", "hug", "hugging");
		add("hud.reactions_and_symbols", "skull");
		add("hud.reactions_and_symbols", "heart");
		add("hud.reactions_and_symbols", "blue_heart");
		add("hud.reactions_and_symbols", "fire");
		add("hud.reactions_and_symbols", "thumbsup", "+1");
		add("hud.reactions_and_symbols", "thumbsdown", "-1");
		add("hud.reactions_and_symbols", "clap");
		add("hud.reactions_and_symbols", "wave");
		add("hud.reactions_and_symbols", "eyes");
		add("hud.reactions_and_symbols", "pray");
		add("hud.reactions_and_symbols", "check", "white_check_mark");
		add("hud.reactions_and_symbols", "x", "cross");
		add("hud.reactions_and_symbols", "star");
		add("hud.reactions_and_symbols", "sparkles");
		add("hud.reactions_and_symbols", "zzz");
		add("hud.reactions_and_symbols", "crown");
		add("hud.reactions_and_symbols", "ok_hand");
		add("hud.reactions_and_symbols", "muscle");
		add("hud.reactions_and_symbols", "raised_hands");
		add("hud.reactions_and_symbols", "broken_heart");
		add("hud.reactions_and_symbols", "100");
		add("hud.reactions_and_symbols", "tada");
		add("hud.reactions_and_symbols", "trophy");
		add("hud.reactions_and_symbols", "crossed_swords", "swords");
		add("hud.reactions_and_symbols", "gem");
		add("hud.reactions_and_symbols", "boom");
		add("hud.reactions_and_symbols", "ghost");
		add("hud.reactions_and_symbols", "pick", "pickaxe");
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
