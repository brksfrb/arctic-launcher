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
	/** Fancy mode picked with the style in the launcher, null if unknown. */
	public final Boolean fancy;
	/** The launcher's proxy (null if it sent none) and when it was set there. */
	public final ProxyConfig proxy;
	public final long proxySet;
	/** The launcher's account-switching bridge (port 0 = none). */
	public int bridgePort;
	/** Friends may see which server we're on. */
	public boolean shareServer;
	public String bridgeSecret;
	/** When the style was picked in the launcher (seconds), 0 if unknown. */
	public final long styleSet;
	/** A duel the launcher asked for: kit, friend (may be null), and when (seconds). */
	public String duelKit;
	public String duelFriend;
	public String duelFriendName;
	public long duelAt;
	/** A replay the launcher asked to open, and when (seconds). */
	public String replayFile;
	public long replayAt;

	private Session(String url, String token, String style, Boolean fancy, long styleSet, ProxyConfig proxy, long proxySet) {
		this.proxy = proxy;
		this.proxySet = proxySet;
		this.url = url;
		this.token = token;
		this.style = style;
		this.fancy = fancy;
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
			JsonElement p = o.get("proxy");
			ProxyConfig proxy = null;
			long proxySet = 0;
			if (p != null && p.isJsonObject()) {
				JsonObject po = p.getAsJsonObject();
				proxy = new ProxyConfig();
				Boolean on = bool(po, "enabled");
				proxy.enabled = on != null && on;
				proxy.host = string(po, "host");
				proxy.port = (int) number(po, "port");
				proxy.username = string(po, "username");
				proxy.password = string(po, "password");
				proxy.fillDefaults();
				proxySet = number(po, "set");
			}
			Session session = new Session(string(o, "url"), string(o, "token"), string(o, "style"), bool(o, "fancy"),
					number(o, "style_set"), proxy, proxySet);
			Boolean share = bool(o, "share_server");
			session.shareServer = share != null && share;
			JsonElement b = o.get("bridge");
			if (b != null && b.isJsonObject()) {
				long port = number(b.getAsJsonObject(), "port");
				session.bridgePort = port > 0 && port < 65536 ? (int) port : 0;
				session.bridgeSecret = string(b.getAsJsonObject(), "secret");
			}
			JsonElement r = o.get("replay");
			if (r != null && r.isJsonObject()) {
				session.replayFile = string(r.getAsJsonObject(), "file");
				session.replayAt = number(r.getAsJsonObject(), "at");
			}
			JsonElement d = o.get("duel");
			if (d != null && d.isJsonObject()) {
				JsonObject duel = d.getAsJsonObject();
				session.duelKit = string(duel, "kit");
				session.duelFriend = string(duel, "friend");
				session.duelFriendName = string(duel, "friend_name");
				session.duelAt = number(duel, "at");
			}
			return session;
		} catch (Exception e) {
			return empty();
		}
	}

	private static Session empty() {
		return new Session(null, null, null, null, 0, null, 0);
	}

	private static String string(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? null : e.getAsString();
	}

	private static Boolean bool(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? null : e.getAsBoolean();
	}

	private static long number(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? 0 : e.getAsLong();
	}
}
