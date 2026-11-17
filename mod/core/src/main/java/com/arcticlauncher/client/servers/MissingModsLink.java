package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The missing-mods install through Arctic Launcher, over its local bridge: the game sends what the
 * server named, the launcher looks the mods up on Modrinth and asks the player there (nothing is
 * installed without a yes), then the game asks how it went. "Restart and rejoin" tells the launcher to
 * start this instance again straight into the server once the game has quit.
 */
public final class MissingModsLink {
	private static final int MAX_REASON = 2000;

	private final String base;
	private final String secret;
	private final String launch;
	private final Gson gson = new Gson();

	/** How the install goes, as the launcher last said. */
	public static final class State {
		/** {@code idle}, {@code looking}, {@code review}, {@code installing}, {@code installed}, {@code declined}, {@code failed}, or {@code offline}. */
		public final String state;
		public final int installed;
		public final List<String> notFound;
		public final String message;

		State(String state, int installed, List<String> notFound, String message) {
			this.state = state;
			this.installed = installed;
			this.notFound = notFound;
			this.message = message;
		}

		static final State OFFLINE = new State("offline", 0, Collections.<String>emptyList(), null);
	}

	public MissingModsLink(int bridgePort, String secret, String launchToken) {
		boolean ok = bridgePort > 0 && secret != null && launchToken != null && !launchToken.isEmpty();
		this.base = ok ? "http://127.0.0.1:" + bridgePort + "/v1/missing-mods" : null;
		this.secret = secret;
		this.launch = launchToken;
	}

	/** Started from Arctic Launcher (which can install mods). */
	public boolean available() {
		return base != null;
	}

	/** The request body: the server, its mods and what it said. */
	static String body(String launch, String server, MissingMods.Info info, String reason) {
		JsonObject body = new JsonObject();
		body.addProperty("launch", launch);
		body.addProperty("server", server == null ? "" : server);
		JsonArray mods = new JsonArray();
		for (MissingMods.Entry e : info.mods) {
			JsonObject m = new JsonObject();
			m.addProperty("id", e.id);
			if (e.version != null) {
				m.addProperty("version", e.version);
			}
			mods.add(m);
		}
		body.add("mods", mods);
		if (info.loader != null) {
			body.addProperty("loader", info.loader);
		}
		String r = reason == null ? "" : reason;
		body.addProperty("reason", r.length() > MAX_REASON ? r.substring(0, MAX_REASON) : r);
		return body.toString();
	}

	/** Ask the launcher to install (call off the game thread). */
	public State request(String server, MissingMods.Info info, String reason) {
		if (base == null) {
			return State.OFFLINE;
		}
		try {
			return parse(Http.send("POST", base, secret, body(launch, server, info, reason)));
		} catch (Exception e) {
			return new State("failed", 0, Collections.<String>emptyList(), e.getMessage());
		}
	}

	/** How it is going (call off the game thread). */
	public State poll() {
		if (base == null) {
			return State.OFFLINE;
		}
		try {
			return parse(Http.send("GET", base + "/" + launch, secret, null));
		} catch (Exception e) {
			return State.OFFLINE;
		}
	}

	/** Tell the launcher to start the game again into the server after it quits. */
	public boolean restart() {
		if (base == null) {
			return false;
		}
		try {
			Http.send("POST", base + "/" + launch + "/restart", secret, "{}");
			return true;
		} catch (Exception e) {
			return false;
		}
	}

	State parse(String json) {
		JsonObject o = gson.fromJson(json, JsonObject.class);
		if (o == null) {
			return State.OFFLINE;
		}
		List<String> notFound = new ArrayList<String>();
		JsonElement nf = o.get("not_found");
		if (nf != null && nf.isJsonArray()) {
			for (JsonElement e : nf.getAsJsonArray()) {
				if (e.isJsonPrimitive()) {
					notFound.add(e.getAsString());
				}
			}
		}
		String state = o.has("state") && o.get("state").isJsonPrimitive() ? o.get("state").getAsString() : "idle";
		int installed = o.has("installed") && o.get("installed").isJsonPrimitive() ? o.get("installed").getAsInt() : 0;
		String message = o.has("message") && o.get("message").isJsonPrimitive() ? o.get("message").getAsString() : null;
		return new State(state, installed, notFound, message);
	}
}
