package com.arcticlauncher.client;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * The Arctic Client's text in the player's language.
 *
 * <p>Every piece of text the client shows goes through {@link #t(String)} (with {@code {0}},
 * {@code {1}}… placeholders through {@link #t(String, Object...)}) or {@link #plural}. English is
 * the source: {@code assets/arctic/lang/en/<area>.json} holds every key, and
 * {@code assets/arctic/lang/<code>/<area>.json} the translations, flat {@code "key": "text"}
 * objects. Text missing from a language falls back to English, and a key missing everywhere
 * shows as itself, so nothing is ever blank.
 *
 * <p>The language is chosen once, when the client starts: the launcher's pick (its session
 * file), else the game's own language, else English. See docs/i18n.md.
 */
public final class Lang {
	/** The files each language is split into (one per part of the client). */
	public static final String[] AREAS = {"common", "mods", "hud", "social", "looks", "servers", "replay", "platform"};
	/** The languages the client speaks, by their folder under {@code assets/arctic/lang}. */
	public static final String[] CODES = {"en", "tr", "de", "fr", "es", "pt", "it", "nl", "pl", "ru", "uk", "zh", "ja", "ko"};

	private static final Map<String, String> EMPTY = Collections.emptyMap();
	private static volatile Map<String, String> english = EMPTY;
	private static volatile Map<String, String> current = EMPTY;
	private static volatile String code = "en";
	private static Platform platform;

	/** What the launcher asked for, and the game's language as last seen. */
	private static String launcherPick;
	private static String lastGame;

	private Lang() {}

	/**
	 * Pick the language and load its text: {@code launcherLanguage} (a code the launcher sent,
	 * "" or null to follow the game) wins, then the game's language, then English.
	 */
	public static void init(Platform p, String launcherLanguage) {
		platform = p;
		english = load("en");
		String game = null;
		try {
			game = p == null ? null : p.gameLanguage();
		} catch (RuntimeException e) {
			// A version whose options aren't ready yet: English.
		}
		launcherPick = launcherLanguage;
		lastGame = game;
		use(choose(launcherLanguage, game));
	}

	/**
	 * Call often (every tick): when the game's language changes in Minecraft's options, the
	 * client switches with it. The launcher's pick holds until then. True when the language
	 * changed, so text worked out ahead of time (the HUD's) can be made again.
	 */
	public static boolean follow() {
		if (platform == null) {
			return false;
		}
		String game;
		try {
			game = platform.gameLanguage();
		} catch (RuntimeException e) {
			return false;
		}
		if (game == null || game.equals(lastGame)) {
			return false;
		}
		// The first value seen (options read after start-up) keeps the launcher's pick.
		boolean firstSeen = lastGame == null;
		lastGame = game;
		if (firstSeen && supported(launcherPick) != null) {
			return false;
		}
		String before = code;
		use(choose(null, game));
		return !code.equals(before);
	}

	/** Switch to {@code wanted} (a supported code) from now on. */
	public static void use(String wanted) {
		String resolved = supported(wanted);
		code = resolved == null ? "en" : resolved;
		current = code.equals("en") ? english : load(code);
	}

	/** The code in use ("en" until {@link #init}). */
	public static String code() {
		return code;
	}

	/** The launcher's pick if it's a language we speak, else the game's, else English. */
	static String choose(String launcher, String game) {
		String c = supported(launcher);
		if (c != null) {
			return c;
		}
		c = supported(game);
		return c != null ? c : "en";
	}

	/** "tr", "tr_TR", "tr-tr", "pt_br", "zh_cn" → the supported code, or null. */
	static String supported(String tag) {
		if (tag == null) {
			return null;
		}
		String primary = tag.trim().toLowerCase(Locale.ROOT).split("[-_.@]")[0];
		for (String c : CODES) {
			if (c.equals(primary)) {
				return c;
			}
		}
		return null;
	}

	/** Text for {@code key} in the current language, else English, else the key itself. */
	public static String t(String key) {
		String s = get(key);
		return s == null ? key : s;
	}

	/** {@link #t(String)} with {@code {0}}, {@code {1}}… replaced by {@code args}. */
	public static String t(String key, Object... args) {
		return fill(t(key), args);
	}

	/** The text of each key, in order (for a table of choices kept as keys). */
	public static String[] all(String... keys) {
		String[] out = new String[keys.length];
		for (int i = 0; i < keys.length; i++) {
			out[i] = t(keys[i]);
		}
		return out;
	}

	/** Text for a key, or null when no language has it. */
	public static String lookup(String key) {
		return get(key);
	}

	/**
	 * Text for a count: {@code key.one}, {@code key.few}, {@code key.many} or {@code key.other}
	 * by the language's rules, with {@code {n}} (and {@code {0}}… from {@code args}) filled in.
	 */
	public static String plural(String key, long n, Object... args) {
		String form = pluralForm(code, n);
		String s = get(key + "." + form);
		if (s == null) {
			s = get(key + ".other");
		}
		if (s == null) {
			s = t(key);
		}
		return fill(s.replace("{n}", String.valueOf(n)), args);
	}

	private static String get(String key) {
		String s = current.get(key);
		return s != null ? s : english.get(key);
	}

	/** {@code {0}}, {@code {1}}… in {@code template} replaced; a placeholder with no argument stays. */
	static String fill(String template, Object[] args) {
		if (args == null || args.length == 0 || template.indexOf('{') < 0) {
			return template;
		}
		StringBuilder out = new StringBuilder(template.length() + 16);
		int i = 0;
		while (i < template.length()) {
			char c = template.charAt(i);
			if (c == '{') {
				int end = template.indexOf('}', i);
				if (end > i) {
					int index = index(template, i + 1, end);
					if (index >= 0 && index < args.length) {
						out.append(args[index]);
						i = end + 1;
						continue;
					}
				}
			}
			out.append(c);
			i++;
		}
		return out.toString();
	}

	/** The digits between {@code from} and {@code to} as a number, or -1 when they're not digits. */
	private static int index(String s, int from, int to) {
		if (from >= to || to - from > 3) {
			return -1;
		}
		int n = 0;
		for (int i = from; i < to; i++) {
			char c = s.charAt(i);
			if (c < '0' || c > '9') {
				return -1;
			}
			n = n * 10 + (c - '0');
		}
		return n;
	}

	/** The CLDR plural category of {@code n} in a language. */
	static String pluralForm(String code, long n) {
		n = Math.abs(n);
		long tens = n % 10;
		long hundreds = n % 100;
		boolean few = tens >= 2 && tens <= 4 && !(hundreds >= 12 && hundreds <= 14);
		if ("ru".equals(code) || "uk".equals(code)) {
			return tens == 1 && hundreds != 11 ? "one" : few ? "few" : "many";
		}
		if ("pl".equals(code)) {
			return n == 1 ? "one" : few ? "few" : "many";
		}
		if ("fr".equals(code) || "pt".equals(code)) {
			return n <= 1 ? "one" : "other";
		}
		if ("zh".equals(code) || "ja".equals(code) || "ko".equals(code)) {
			return "other";
		}
		return n == 1 ? "one" : "other";
	}

	/** Every {@code assets/arctic/lang/<code>/<area>.json} on the classpath, merged. */
	static Map<String, String> load(String code) {
		Map<String, String> out = new HashMap<String, String>();
		for (String area : AREAS) {
			String path = "/assets/arctic/lang/" + code + "/" + area + ".json";
			InputStream in = Lang.class.getResourceAsStream(path);
			if (in == null) {
				continue;
			}
			try {
				Reader reader = new InputStreamReader(in, StandardCharsets.UTF_8);
				JsonObject o = new Gson().fromJson(reader, JsonObject.class);
				if (o != null) {
					for (Map.Entry<String, JsonElement> e : o.entrySet()) {
						if (e.getValue().isJsonPrimitive()) {
							out.put(e.getKey(), e.getValue().getAsString());
						}
					}
				}
			} catch (RuntimeException e) {
				if (platform != null) {
					platform.log(true, "Arctic text " + path + " is broken: " + e);
				}
			} finally {
				try {
					in.close();
				} catch (IOException ignored) {
					// Nothing to do about a classpath stream that won't close.
				}
			}
		}
		return out;
	}
}
