package com.arcticlauncher.client.feature;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Symbols the game's font can draw, and ":heart:"-style shortcodes for typing them: chat has
 * no color emoji, but these show the same on every version.
 */
public final class Emoji {
	/** A shortcode's longest length (so a lone ":" is never scanned far). */
	private static final int MAX_CODE = 20;

	private static final Map<String, String> CODES = new LinkedHashMap<String, String>();
	/** Name of a group, then its symbols separated by spaces (the picker). */
	private static final Map<String, String> GROUPS = new LinkedHashMap<String, String>();

	private static void add(String code, String symbol) {
		CODES.put(code, symbol);
	}

	static {
		add("heart", "♥");
		add("heart_empty", "♡");
		add("star", "★");
		add("star_empty", "☆");
		add("check", "✔");
		add("tick", "✓");
		add("x", "✖");
		add("cross", "✗");
		add("smile", "☺");
		add("smile_dark", "☻");
		add("sad", "☹");
		add("skull", "☠");
		add("peace", "☮");
		add("yinyang", "☯");
		add("music", "♪");
		add("notes", "♫");
		add("flower", "✿");
		add("blossom", "❀");
		add("snow", "❄");
		add("snowflake", "❅");
		add("sun", "☀");
		add("cloud", "☁");
		add("umbrella", "☂");
		add("snowman", "☃");
		add("swords", "⚔");
		add("bolt", "⚡");
		add("anchor", "⚓");
		add("plane", "✈");
		add("phone", "☎");
		add("mail", "✉");
		add("scissors", "✂");
		add("pencil", "✎");
		add("arrow", "➔");
		add("arrow_right", "→");
		add("arrow_left", "←");
		add("arrow_up", "↑");
		add("arrow_down", "↓");
		add("arrow_both", "↔");
		add("double_right", "⇒");
		add("spade", "♠");
		add("club", "♣");
		add("diamond", "♦");
		add("infinity", "∞");
		add("female", "♀");
		add("male", "♂");
		add("coffee", "☕");
		add("hot", "♨");
		add("copyright", "©");
		add("degree", "°");
		add("plus_minus", "±");
		add("half", "½");
		add("square", "■");
		add("circle", "●");
		add("circle_empty", "○");
		add("triangle", "▲");
		add("triangle_down", "▼");
		add("play", "▶");
		add("dot", "•");
		add("flag", "⚑");
		add("warning", "⚠");
		add("hammer", "⚒");
		add("crown", "♛");
		add("king", "♔");
		add("chess", "♟");
		add("moon", "☾");
		add("comet", "☄");
		add("gear", "⚙");
		add("recycle", "♻");
		add("lambda", "λ");
		add("pi", "π");
		add("sigma", "Σ");
		add("omega", "Ω");
		add("euro", "€");
		add("pound", "£");
		add("yen", "¥");
		add("trademark", "™");

		GROUPS.put("Hearts and stars", "♥ ♡ ★ ☆ ✿ ❀ ❄ ❅ ✵ ✶ ✷ ✽");
		GROUPS.put("Faces and signs", "☺ ☻ ☹ ☠ ☮ ☯ ♀ ♂ ✔ ✓ ✖ ✗");
		GROUPS.put("Weather and nature", "☀ ☁ ☂ ☃ ⚡ ☾ ☄ ☕ ♨ ♻");
		GROUPS.put("Arrows", "← ↑ → ↓ ↔ ↕ ⇒ ⇐ ⇔ ➔ ➤ ↺");
		GROUPS.put("Games and music", "♠ ♣ ♥ ♦ ♛ ♔ ♟ ♪ ♫ ♬ ⚔ ⚒");
		GROUPS.put("Shapes and marks", "■ □ ● ○ ▲ ▼ ▶ ◀ • ‣ ⚑ ⚠");
		GROUPS.put("Letters and math", "∞ ± ½ ° π Σ Ω λ © ™ € £");
	}

	private Emoji() {}

	/** Group names with their symbols (space separated), in order. */
	public static Map<String, String> groups() {
		return GROUPS;
	}

	/** Shortcode ("heart") to symbol, in order. */
	public static Map<String, String> codes() {
		return CODES;
	}

	/** {@code text} with each ":name:" turned into its symbol; unknown names stay as typed. */
	public static String expand(String text) {
		if (text == null || text.indexOf(':') < 0) {
			return text;
		}
		StringBuilder out = null;
		int i = 0;
		int n = text.length();
		while (i < n) {
			char c = text.charAt(i);
			if (c == ':') {
				int end = text.indexOf(':', i + 1);
				if (end > i + 1 && end - i - 1 <= MAX_CODE) {
					String symbol = CODES.get(text.substring(i + 1, end).toLowerCase(Locale.ROOT));
					if (symbol != null) {
						if (out == null) {
							out = new StringBuilder(n).append(text, 0, i);
						}
						out.append(symbol);
						i = end + 1;
						continue;
					}
				}
			}
			if (out != null) {
				out.append(c);
			}
			i++;
		}
		return out == null ? text : out.toString();
	}
}
