package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.google.gson.JsonObject;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/**
 * Where the player's time goes, for the launcher's Stats page: each stretch spent in a server, in a
 * singleplayer world or in the menus is appended to {@code arctic-playtime.jsonl} in the game folder
 * as {@code {"k":"server|world|menu","n":"<address or world>","s":<start>,"e":<end>}} (Unix seconds).
 * The open stretch is written every minute, so a crash loses at most that; the launcher reads and
 * clears the file when the game closes.
 */
public final class PlaytimeLog {
	private static final String FILE = "arctic-playtime.jsonl";
	private static final long FLUSH_MS = 60_000;
	/** A server address or world name longer than this is cut (nothing sensible is longer). */
	private static final int MAX_NAME = 120;

	private String kind = "";
	private String name = "";
	private long since;
	private File file;
	private boolean hooked;

	/** Every client tick: note where the player is now, and write a finished stretch. */
	public synchronized void tick(Platform platform) {
		long now = System.currentTimeMillis();
		if (!hooked) {
			hooked = true;
			file = new File(platform.configDir().getParentFile(), FILE);
			// The game closing writes the stretch that is still open.
			Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
				@Override
				public void run() {
					close();
				}
			}, "arctic-playtime"));
		}
		String nextKind;
		String nextName = "";
		if (!platform.inWorld()) {
			nextKind = "menu";
		} else {
			String server = platform.server();
			if (server == null || "Singleplayer".equals(server)) {
				nextKind = "world";
				String key = platform.worldKey();
				nextName = key == null ? "" : key;
			} else {
				nextKind = "server";
				nextName = server;
			}
		}
		if (nextName.length() > MAX_NAME) {
			nextName = nextName.substring(0, MAX_NAME);
		}
		if (!nextKind.equals(kind) || !nextName.equals(name)) {
			flush(now);
			kind = nextKind;
			name = nextName;
			since = now;
		} else if (now - since >= FLUSH_MS) {
			flush(now);
			since = now;
		}
	}

	/** Write the stretch that is open now (the game is closing). */
	public synchronized void close() {
		flush(System.currentTimeMillis());
		kind = "";
	}

	private void flush(long now) {
		if (kind.isEmpty() || file == null || now - since < 1000) {
			return;
		}
		JsonObject line = new JsonObject();
		line.addProperty("k", kind);
		line.addProperty("n", name);
		line.addProperty("s", since / 1000);
		line.addProperty("e", now / 1000);
		try (FileOutputStream out = new FileOutputStream(file, true)) {
			out.write((line.toString() + "\n").getBytes(StandardCharsets.UTF_8));
		} catch (IOException e) {
			// Play time is a nicety: never let it hurt the game.
		}
	}
}
