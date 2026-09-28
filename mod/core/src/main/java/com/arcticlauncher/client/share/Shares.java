package com.arcticlauncher.client.share;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.CrosshairConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.DataFormatException;
import java.util.zip.Deflater;
import java.util.zip.Inflater;

/**
 * HUD layouts and crosshairs as share bundles, the same format the launcher
 * uses ({@code {"arctic_share":1,"kind":…,"data":…}}): as a short code on
 * the Arctic server or one line of {@code arctic1.} text. Every bundle read
 * is checked value by value (the same limits as the launcher and the
 * in-game editors) before anything changes.
 */
public final class Shares {
	public static final String HUD = "hud";
	public static final String CROSSHAIR = "crosshair";
	public static final String CLIENT = "client";
	private static final int FORMAT = 1;
	private static final String PREFIX = "arctic1.";
	private static final int MAX_JSON = 256 * 1024;
	private static final String ALPHABET = "abcdefghjkmnpqrstuvwxyz23456789";
	private static final int CODE_LEN = 8;
	private static final int MAX_WIDGETS = 64;
	private static final int MAX_WIDGET_ID = 40;
	private static final int MAX_KEY = 64;
	private static final int MAX_OFFSET = 4096;
	private static final int MAX_HUD_VERSION = 100;
	private static final float MIN_SCALE = 0.5f;
	private static final float MAX_SCALE = 2.5f;
	private static final List<String> STYLES = Arrays.asList("arctic", "aurora", "classic");
	private static final String[] BOOLEANS = {"fancy", "showCosmetics", "fullbright", "zoomEnabled", "freelookEnabled",
			"toggleSprint", "toggleSneak", "chatTimestamps", "chatStack", "lowFire", "clearWeather", "confirmLeave"};
	private static final String[] KEYS = {"zoomKey", "freelookKey", "fullbrightKey", "emoteKey"};
	private static final Gson GSON = new Gson();

	private Shares() {}

	/** A bundle of this part of the settings ({@link #HUD} or {@link #CROSSHAIR}). */
	public static JsonObject bundle(String kind, ClientConfig config) {
		JsonObject all = GSON.toJsonTree(config).getAsJsonObject();
		JsonObject data = new JsonObject();
		if (HUD.equals(kind)) {
			data.add("hud", all.get("hud"));
			data.add("hudVersion", all.get("hudVersion"));
		} else {
			data.add("crosshair", all.get("crosshair"));
		}
		JsonObject bundle = new JsonObject();
		bundle.addProperty("arctic_share", FORMAT);
		bundle.addProperty("kind", kind);
		bundle.add("data", data);
		return bundle;
	}

	/** One line of text: {@code arctic1.} + base64 of the deflated JSON. */
	public static String toText(JsonObject bundle) {
		byte[] json = bundle.toString().getBytes(StandardCharsets.UTF_8);
		Deflater deflater = new Deflater(Deflater.BEST_COMPRESSION, true);
		deflater.setInput(json);
		deflater.finish();
		ByteArrayOutputStream out = new ByteArrayOutputStream();
		byte[] buf = new byte[4096];
		while (!deflater.finished()) {
			out.write(buf, 0, deflater.deflate(buf));
		}
		deflater.end();
		return PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(out.toByteArray());
	}

	/** The code as the server stores it, or null if it can't be one. */
	public static String cleanCode(String input) {
		StringBuilder b = new StringBuilder();
		for (char c : input.trim().toCharArray()) {
			if (c != '-' && !Character.isWhitespace(c)) {
				b.append(Character.toLowerCase(c));
			}
		}
		String code = b.toString();
		if (code.length() != CODE_LEN) {
			return null;
		}
		for (char c : code.toCharArray()) {
			if (ALPHABET.indexOf(c) < 0) {
				return null;
			}
		}
		return code;
	}

	/** {@code abcdefgh} → {@code abcd-efgh}. */
	public static String pretty(String code) {
		return code.substring(0, CODE_LEN / 2) + "-" + code.substring(CODE_LEN / 2);
	}

	public static boolean isText(String input) {
		return input.trim().startsWith(PREFIX);
	}

	/** Parse {@code arctic1.} text or bundle JSON. */
	public static JsonObject parse(String input) throws ShareException {
		String s = input.trim();
		if (s.length() > MAX_JSON * 2) {
			throw new ShareException("That's too long to be a share.");
		}
		String json = s.startsWith(PREFIX) ? inflate(s.substring(PREFIX.length())) : s;
		try {
			JsonElement e = GSON.fromJson(json, JsonElement.class);
			if (e == null || !e.isJsonObject()) {
				throw new ShareException("That isn't something Arctic shared.");
			}
			return e.getAsJsonObject();
		} catch (RuntimeException e) {
			throw new ShareException("That share is damaged.");
		}
	}

	private static String inflate(String packed) throws ShareException {
		byte[] bytes;
		try {
			bytes = Base64.getUrlDecoder().decode(packed.replaceAll("\\s", ""));
		} catch (IllegalArgumentException e) {
			throw new ShareException("That share text is damaged or cut off.");
		}
		Inflater inflater = new Inflater(true);
		inflater.setInput(bytes);
		ByteArrayOutputStream out = new ByteArrayOutputStream();
		byte[] buf = new byte[4096];
		try {
			while (!inflater.finished()) {
				int n = inflater.inflate(buf);
				if (n == 0 && (inflater.needsInput() || inflater.needsDictionary())) {
					break;
				}
				out.write(buf, 0, n);
				if (out.size() > MAX_JSON) {
					throw new ShareException("That share is too big.");
				}
			}
		} catch (DataFormatException e) {
			throw new ShareException("That share text is damaged or cut off.");
		} finally {
			inflater.end();
		}
		return new String(out.toByteArray(), StandardCharsets.UTF_8);
	}

	/**
	 * Check a bundle and apply it to {@code config}; returns what changed.
	 * Nothing changes unless every value is fine.
	 */
	public static String apply(JsonObject bundle, ClientConfig config) throws ShareException {
		JsonElement format = bundle.get("arctic_share");
		if (!isInt(format, 1, Integer.MAX_VALUE)) {
			throw new ShareException("That isn't something Arctic shared.");
		}
		if (format.getAsInt() > FORMAT) {
			throw new ShareException("It's from a newer Arctic; update to use it.");
		}
		String kind = string(bundle.get("kind"));
		JsonElement dataEl = bundle.get("data");
		if (!(dataEl != null && dataEl.isJsonObject())) {
			throw new ShareException("That share is damaged.");
		}
		JsonObject data = dataEl.getAsJsonObject();
		if ("instance".equals(kind) || "profile".equals(kind)) {
			throw new ShareException("That's an instance or profile: import it in Arctic Launcher.");
		}
		if (!(HUD.equals(kind) || CROSSHAIR.equals(kind) || CLIENT.equals(kind))) {
			throw new ShareException("This Arctic doesn't know that kind of share; try updating.");
		}
		boolean all = CLIENT.equals(kind);
		Map<String, HudSlot> hud = null;
		Integer hudVersion = null;
		CrosshairConfig crosshair = null;
		if (HUD.equals(kind) || all) {
			hud = data.has("hud") ? hud(data.get("hud")) : null;
			if (data.has("hudVersion")) {
				if (!isInt(data.get("hudVersion"), 1, MAX_HUD_VERSION)) {
					throw bad("hudVersion");
				}
				hudVersion = data.get("hudVersion").getAsInt();
			}
		}
		if (CROSSHAIR.equals(kind) || all) {
			crosshair = data.has("crosshair") ? crosshair(data.get("crosshair")) : null;
		}
		Map<String, Object> rest = all ? rest(data) : new LinkedHashMap<String, Object>();
		if (hud == null && crosshair == null && rest.isEmpty()) {
			throw new ShareException("There's nothing in that share.");
		}
		if (hud != null) {
			config.hud.clear();
			config.hud.putAll(hud);
			if (hudVersion != null) {
				config.hudVersion = hudVersion;
			}
		}
		if (crosshair != null) {
			config.crosshair = crosshair;
		}
		for (Map.Entry<String, Object> e : rest.entrySet()) {
			set(config, e.getKey(), e.getValue());
		}
		if (all) {
			return "Applied the client settings.";
		}
		return hud != null ? "Applied the HUD layout." : "Applied the crosshair.";
	}

	private static Map<String, HudSlot> hud(JsonElement el) throws ShareException {
		if (!el.isJsonObject() || el.getAsJsonObject().entrySet().size() > MAX_WIDGETS) {
			throw bad("hud");
		}
		Map<String, HudSlot> out = new LinkedHashMap<String, HudSlot>();
		for (Map.Entry<String, JsonElement> e : el.getAsJsonObject().entrySet()) {
			if (!isWidgetId(e.getKey()) || !e.getValue().isJsonObject()) {
				throw bad("hud");
			}
			JsonObject s = e.getValue().getAsJsonObject();
			HudSlot slot = new HudSlot(bool(s, "enabled", false));
			slot.placed = bool(s, "placed", false);
			slot.ax = integer(s, "ax", 0, 2, 0);
			slot.ay = integer(s, "ay", 0, 2, 0);
			slot.dx = integer(s, "dx", -MAX_OFFSET, MAX_OFFSET, 0);
			slot.dy = integer(s, "dy", -MAX_OFFSET, MAX_OFFSET, 0);
			slot.background = bool(s, "background", true);
			if (s.has("scale")) {
				JsonElement sc = s.get("scale");
				if (!isNumber(sc) || sc.getAsFloat() < MIN_SCALE || sc.getAsFloat() > MAX_SCALE) {
					throw bad("hud");
				}
				slot.scale = sc.getAsFloat();
			}
			out.put(e.getKey(), slot);
		}
		return out;
	}

	private static CrosshairConfig crosshair(JsonElement el) throws ShareException {
		if (!el.isJsonObject()) {
			throw bad("crosshair");
		}
		JsonObject o = el.getAsJsonObject();
		CrosshairConfig c = new CrosshairConfig();
		c.enabled = bool(o, "enabled", false);
		if (o.has("style")) {
			String style = string(o.get("style"));
			if (!Arrays.asList(CrosshairConfig.STYLES).contains(style)) {
				throw bad("crosshair");
			}
			c.style = style;
		}
		c.size = integer(o, "size", 1, 12, 5);
		c.gap = integer(o, "gap", 0, 8, 2);
		c.thickness = integer(o, "thickness", 1, 4, 1);
		c.color = integer(o, "color", Integer.MIN_VALUE, Integer.MAX_VALUE, 0xFFFFFFFF);
		c.outline = bool(o, "outline", true);
		return c;
	}

	/** Style, feature switches and keys of a "client" bundle. */
	private static Map<String, Object> rest(JsonObject data) throws ShareException {
		Map<String, Object> out = new LinkedHashMap<String, Object>();
		for (String k : BOOLEANS) {
			if (data.has(k)) {
				out.put(k, bool(data, k, false));
			}
		}
		for (String k : KEYS) {
			if (data.has(k)) {
				String key = string(data.get(k));
				if (!isKeyName(key)) {
					throw bad(k);
				}
				out.put(k, key);
			}
		}
		if (data.has("style")) {
			String style = string(data.get("style"));
			if (!STYLES.contains(style)) {
				throw bad("style");
			}
			out.put("style", style);
		}
		return out;
	}

	/** Set a checked field (names from {@link ClientConfig}). */
	private static void set(ClientConfig c, String key, Object v) {
		try {
			java.lang.reflect.Field f = ClientConfig.class.getField(key);
			f.set(c, v);
		} catch (ReflectiveOperationException e) {
			// Only known field names get here.
		}
	}

	private static boolean isWidgetId(String id) {
		if (id.isEmpty() || id.length() > MAX_WIDGET_ID) {
			return false;
		}
		for (char c : id.toCharArray()) {
			if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '.' || c == '-')) {
				return false;
			}
		}
		return true;
	}

	private static boolean isKeyName(String s) {
		if (s == null || s.length() > MAX_KEY || !(s.startsWith("key.keyboard.") || s.startsWith("key.mouse."))) {
			return false;
		}
		for (char c : s.toCharArray()) {
			if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '.' || c == '_')) {
				return false;
			}
		}
		return true;
	}

	private static boolean bool(JsonObject o, String k, boolean fallback) throws ShareException {
		if (!o.has(k)) {
			return fallback;
		}
		JsonElement e = o.get(k);
		if (!(e.isJsonPrimitive() && e.getAsJsonPrimitive().isBoolean())) {
			throw bad(k);
		}
		return e.getAsBoolean();
	}

	private static int integer(JsonObject o, String k, int lo, int hi, int fallback) throws ShareException {
		if (!o.has(k)) {
			return fallback;
		}
		if (!isInt(o.get(k), lo, hi)) {
			throw bad(k);
		}
		return o.get(k).getAsInt();
	}

	private static boolean isNumber(JsonElement e) {
		return e != null && e.isJsonPrimitive() && e.getAsJsonPrimitive().isNumber();
	}

	private static boolean isInt(JsonElement e, long lo, long hi) {
		if (!isNumber(e)) {
			return false;
		}
		double d = e.getAsDouble();
		return d == Math.rint(d) && d >= lo && d <= hi;
	}

	private static String string(JsonElement e) {
		if (e == null || !e.isJsonPrimitive()) {
			return null;
		}
		JsonPrimitive p = e.getAsJsonPrimitive();
		return p.isString() ? p.getAsString() : null;
	}

	private static ShareException bad(String what) {
		return new ShareException("That share has a bad value (" + what + ").");
	}

	/** Why a share can't be used, in words for the player. */
	public static final class ShareException extends Exception {
		private static final long serialVersionUID = 1L;

		public ShareException(String message) {
			super(message);
		}
	}
}
