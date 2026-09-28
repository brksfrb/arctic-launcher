package com.arcticlauncher.client.together;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;

import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonObject;

/**
 * Play together from the game, through Arctic Launcher's local bridge: host
 * the world you opened to LAN, or join a friend's code. The launcher runs
 * the tunnel; this asks it and reads its state back.
 */
public final class TogetherLink {
	private final String url;
	private final String secret;
	private final Gson gson = new Gson();
	private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-together");
		t.setDaemon(true);
		return t;
	});

	private volatile JsonObject state = new JsonObject();

	public TogetherLink(int bridgePort, String secret) {
		this.url = bridgePort > 0 && secret != null ? "http://127.0.0.1:" + bridgePort + "/v1/together" : null;
		this.secret = secret;
	}

	public boolean available() {
		return url != null;
	}

	public void host() {
		ask("host", null);
	}

	public void join(String code) {
		ask("join", code);
	}

	public void stop() {
		ask("stop", null);
	}

	/** Ask for the state now (answers arrive in {@link #state()}). */
	public void refresh() {
		ask("status", null);
	}

	private void ask(final String action, final String code) {
		if (url == null) {
			return;
		}
		worker.execute(() -> {
			JsonObject body = new JsonObject();
			body.addProperty("action", action);
			if (code != null) {
				body.addProperty("code", code);
			}
			try {
				JsonObject reply = gson.fromJson(Http.send("POST", url, secret, body.toString()), JsonObject.class);
				if (reply != null) {
					state = reply;
				}
			} catch (Exception e) {
				JsonObject failed = new JsonObject();
				failed.addProperty("state", "idle");
				failed.addProperty("error", "Arctic Launcher didn't answer: " + e.getMessage());
				state = failed;
			}
		});
	}

	/** "idle", "starting", "hosting", "joining" or "joined". */
	public String stateName() {
		JsonObject s = state;
		return s.has("state") ? s.get("state").getAsString() : "idle";
	}

	/** The invite code while hosting, or null. */
	public String code() {
		JsonObject s = state;
		return s.has("code") && !s.get("code").isJsonNull() ? s.get("code").getAsString() : null;
	}

	/** The local port a joined world is reachable on, or -1. */
	public int port() {
		JsonObject s = state;
		return s.has("port") ? s.get("port").getAsInt() : -1;
	}

	public int guests() {
		JsonObject s = state;
		return s.has("guests") ? s.get("guests").getAsInt() : 0;
	}

	/** The last error, or null. */
	public String error() {
		JsonObject s = state;
		return s.has("error") && !s.get("error").isJsonNull() ? s.get("error").getAsString() : null;
	}
}
