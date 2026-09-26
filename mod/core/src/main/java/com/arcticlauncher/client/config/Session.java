package com.arcticlauncher.client.config;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * What the launcher leaves in {@code config/arctic-session.json} before a
 * launch: the Arctic server, a sign-in token and the chosen menu style.
 */
public final class Session {
	public final String url;
	public final String token;
	public final String style;
	/** When the style was picked in the launcher (seconds), 0 if unknown. */
	public final long styleSet;

	private Session(String url, String token, String style, long styleSet) {
		this.url = url;
		this.token = token;
		this.style = style;
		this.styleSet = styleSet;
	}

	public static Session read(File configDir) {
		File file = new File(configDir, "arctic-session.json");
		try {
			String json = new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
			JsonObject o = new Gson().fromJson(json, JsonObject.class);
			if (o == null) {
				return empty();
			}
			return new Session(string(o, "url"), string(o, "token"), string(o, "style"), number(o, "style_set"));
		} catch (Exception e) {
			return empty();
		}
	}

	private static Session empty() {
		return new Session(null, null, null, 0);
	}

	private static String string(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? null : e.getAsString();
	}

	private static long number(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? 0 : e.getAsLong();
	}
}
