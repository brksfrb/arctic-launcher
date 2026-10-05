package com.arcticlauncher.client.config;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

import com.arcticlauncher.client.Platform;

/**
 * The launcher keeps a game ready in the background (window hidden) and wakes
 * it when Play is pressed. For "join this server" it leaves
 * {@code config/arctic-activate.json}; the game joins and removes the file.
 */
public final class Activation {
	/** The file is looked for this often. */
	private static final long CHECK_MS = 250;
	/** A request older than this (seconds) is a leftover, not a wish. */
	private static final long FRESH_S = 120;
	private static long nextCheck;

	private Activation() {}

	/** Every tick: act on a request the launcher left. */
	public static void poll(Platform platform) {
		long now = System.currentTimeMillis();
		if (now < nextCheck) {
			return;
		}
		nextCheck = now + CHECK_MS;
		File dir = platform.configDir();
		if (dir == null) {
			return;
		}
		File file = new File(dir, "arctic-activate.json");
		if (!file.isFile()) {
			return;
		}
		try {
			JsonObject o = new Gson().fromJson(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8), JsonObject.class);
			file.delete();
			if (o == null || !o.has("server")) {
				return;
			}
			long at = o.has("at") ? o.get("at").getAsLong() : 0;
			String server = o.get("server").getAsString();
			if (!server.isEmpty() && now / 1000 - at < FRESH_S) {
				platform.connectTo(server);
			}
		} catch (Exception e) {
			file.delete();
		}
	}
}
