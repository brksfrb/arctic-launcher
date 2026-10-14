package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.looks.Http;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.File;

/**
 * Finds a video encoder for export: FFmpeg on the PATH, else what Arctic
 * Launcher offers: on Windows the launcher itself (Windows' own H.264
 * encoder, nothing to download), elsewhere a copy of FFmpeg it downloads once.
 */
public final class Ffmpeg {
	private static final long POLL_MS = 500;

	private final String bridgeUrl;
	private final String secret;
	private final boolean nativeOnly;
	private volatile File runner;
	private volatile boolean asking;
	private volatile File found;
	/** {@link #found} is Arctic Launcher's built-in encoder, not FFmpeg. */
	private volatile boolean builtin;
	private volatile String state = "unknown";
	private volatile float progress;
	private volatile String error;
	private volatile long checkedAt;

	public Ffmpeg(int bridgePort, String bridgeSecret) {
		this(bridgePort, bridgeSecret, false);
	}

	public Ffmpeg(int bridgePort, String bridgeSecret, boolean nativeOnly) {
		this.bridgeUrl = bridgePort > 0 && bridgeSecret != null ? "http://127.0.0.1:" + bridgePort : null;
		this.secret = bridgeSecret;
		this.nativeOnly = nativeOnly;
		if (nativeOnly && bridgeUrl != null) ask("GET", null);
	}

	/** FFmpeg's path, or null while it's missing or downloading (then see {@link #state}). */
	public File get() {
		if (found != null) {
			return found;
		}
		String override = System.getProperty("arctic.ffmpeg");
		if (override != null && new File(override).isFile()) {
			found = new File(override);
			return found;
		}
		File onPath = onPath();
		if (onPath != null) {
			found = onPath;
			return found;
		}
		long now = System.currentTimeMillis();
		if (!asking && now - checkedAt > POLL_MS) {
			checkedAt = now;
			ask("GET", null);
		}
		return found;
	}

	/** Is {@code exe} the launcher's built-in encoder (which takes different arguments)? */
	public boolean builtin(File exe) {
		return builtin && exe != null && exe.equals(found);
	}

	/** "missing", "downloading", "ready", "unavailable" (no launcher) or "unknown". */
	public String state() {
		return bridgeUrl == null && found == null ? "unavailable" : state;
	}

	public float progress() {
		return progress;
	}

	public String error() {
		return error;
	}

	/** Ask the launcher to download FFmpeg. */
	public void download() {
		if (asking || "downloading".equals(state)) return;
		state = "downloading";
		progress = 0;
		ask("POST", "{}");
	}

	private void ask(final String method, final String body) {
		if (bridgeUrl == null) {
			return;
		}
		asking = true;
		Thread t = new Thread(() -> {
			try {
				String json = Http.send(method, bridgeUrl + (nativeOnly ? "/v1/ffmpeg/capture" : "/v1/ffmpeg"), secret, body);
				JsonObject o = new JsonParser().parse(json).getAsJsonObject();
				state = o.has("state") ? o.get("state").getAsString() : "missing";
				progress = o.has("progress") ? o.get("progress").getAsFloat() : 0;
				error = o.has("error") && !o.get("error").isJsonNull() ? o.get("error").getAsString() : null;
				if (o.has("runner") && !o.get("runner").isJsonNull()) runner = new File(o.get("runner").getAsString());
				if (o.has("path") && !o.get("path").isJsonNull()) {
					File f = new File(o.get("path").getAsString());
					if (f.isFile()) {
						builtin = o.has("builtin") && o.get("builtin").getAsBoolean();
						found = f;
						state = "ready";
					}
				}
			} catch (Exception e) {
				error = e.getMessage();
			} finally { asking = false; }
		}, "arctic-ffmpeg");
		t.setDaemon(true);
		t.start();
	}

	/** All tool work inherits a background process priority when the launcher is available. */
	ProcessBuilder command(java.util.List<String> args) {
		java.util.List<String> command = new java.util.ArrayList<String>();
		if (runner != null && runner.isFile()) {
			command.add(runner.getAbsolutePath()); command.add("--video-tool");
		} else if (!System.getProperty("os.name", "").toLowerCase().contains("win") && new File("/usr/bin/nice").isFile()) {
			command.add("/usr/bin/nice"); command.add("-n"); command.add("10");
		}
		command.addAll(args);
		return new ProcessBuilder(command);
	}

	private static File onPath() {
		String path = System.getenv("PATH");
		if (path == null) {
			return null;
		}
		String exe = System.getProperty("os.name", "").toLowerCase().contains("win") ? "ffmpeg.exe" : "ffmpeg";
		for (String dir : path.split(File.pathSeparator)) {
			File f = new File(dir, exe);
			if (f.isFile()) {
				return f;
			}
		}
		return null;
	}
}
