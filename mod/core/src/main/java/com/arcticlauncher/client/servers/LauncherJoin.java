package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonObject;

/**
 * "Join" in Arctic Launcher while this game runs: the launcher queues a server address for this game and the
 * game picks it up here. The game always asks (nothing listens in the game), over the launcher's local
 * bridge on 127.0.0.1, with the bridge secret and a token that only this launch has.
 * The connection itself happens on the game's own tick, leaving the current world or server.
 */
public final class LauncherJoin {
	private static final long POLL_MS = 1000;
	/** While the launcher does not answer (it was closed), ask less often. */
	private static final long RETRY_MS = 10_000;
	private static final int MAX_ADDRESS = 255;

	private final String url;
	private final String secret;
	private final Gson gson = new Gson();
	private volatile String pending;

	public LauncherJoin(int bridgePort, String secret, String launchToken) {
		boolean ok = bridgePort > 0 && secret != null && launchToken != null && !launchToken.isEmpty();
		this.url = ok ? "http://127.0.0.1:" + bridgePort + "/v1/join/" + launchToken : null;
		this.secret = secret;
	}

	public boolean available() {
		return url != null;
	}

	public void start() {
		if (url == null) {
			return;
		}
		Thread t = new Thread(this::poll, "arctic-launcher-join");
		t.setDaemon(true);
		t.start();
	}

	private void poll() {
		while (true) {
			long wait = POLL_MS;
			try {
				JsonObject reply = gson.fromJson(Http.send("GET", url, secret, null), JsonObject.class);
				if (reply != null && reply.has("address") && !reply.get("address").isJsonNull()) {
					String address = clean(reply.get("address").getAsString());
					if (address != null) {
						pending = address;
					}
				}
			} catch (Exception e) {
				wait = RETRY_MS;
			}
			try {
				Thread.sleep(wait);
			} catch (InterruptedException e) {
				return;
			}
		}
	}

	/** A server address as typed in the game: one line, no spaces. Null if it is not one. */
	static String clean(String address) {
		if (address == null) {
			return null;
		}
		String a = address.trim();
		if (a.isEmpty() || a.length() > MAX_ADDRESS) {
			return null;
		}
		for (int i = 0; i < a.length(); i++) {
			char c = a.charAt(i);
			if (c <= ' ' || c == 127) {
				return null;
			}
		}
		return a;
	}

	/** Each client tick: connect to the address the launcher sent, leaving what you are in. */
	public void tick(Platform platform) {
		String address = pending;
		if (address == null) {
			return;
		}
		pending = null;
		platform.connectTo(address);
	}
}
