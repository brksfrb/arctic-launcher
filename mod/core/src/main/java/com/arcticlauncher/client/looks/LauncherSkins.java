package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;

/**
 * Your skin library in Arctic Launcher, for picking a skin in the game. The
 * launcher wears it (the same as clicking it there), so it saves and shows
 * the same way.
 */
public final class LauncherSkins {
	/** A skin in the library; {@link #texture} is registered for drawing. */
	public static final class Skin {
		public final String id;
		public final String name;
		public final boolean slim;
		public final String texture;

		Skin(String id, String name, boolean slim, String texture) {
			this.id = id;
			this.name = name;
			this.slim = slim;
			this.texture = texture;
		}
	}

	private static final Gson GSON = new Gson();
	private static final int MAX_SKINS = 60;
	private static final int MAX_NAME = 32;
	/** After wearing one, your look is asked for again this soon (and once more later). */
	private static final long[] RECHECK_MS = {2000, 6000};

	private final Platform platform;
	private final String bridgeUrl;
	private final String secret;
	private volatile List<Skin> skins = Collections.emptyList();
	private volatile boolean loading;
	private volatile boolean loaded;
	private volatile String problem;

	public LauncherSkins(Platform platform, int port, String secret) {
		this.platform = platform;
		this.bridgeUrl = port > 0 && secret != null ? "http://127.0.0.1:" + port : null;
		this.secret = secret;
	}

	public boolean available() {
		return bridgeUrl != null;
	}

	public List<Skin> skins() {
		return skins;
	}

	public boolean loaded() {
		return loaded;
	}

	public String problem() {
		return problem;
	}

	/** Ask the launcher for the library (in the background). */
	public void refresh() {
		if (!available() || loading) {
			return;
		}
		loading = true;
		Thread t = new Thread(() -> {
			try {
				skins = parse(GSON.fromJson(Http.getText(bridgeUrl + "/v1/skins", secret), JsonElement.class));
				problem = null;
			} catch (Exception e) {
				problem = "Couldn't reach Arctic Launcher";
				platform.log(false, "launcher skins: " + e);
			} finally {
				loaded = true;
				loading = false;
			}
		}, "arctic-skins");
		t.setDaemon(true);
		t.start();
	}

	/** Wear a library skin ({@code null}: your Minecraft skin). */
	public void wear(final String id) {
		if (!available()) {
			return;
		}
		Thread t = new Thread(() -> {
			try {
				JsonObject body = new JsonObject();
				body.addProperty("id", id);
				Http.send("POST", bridgeUrl + "/v1/skins/wear", secret, body.toString());
				for (long wait : RECHECK_MS) {
					Thread.sleep(wait);
					ArcticClient.looks().refreshOwn();
				}
			} catch (Exception e) {
				problem = "Couldn't change your skin";
				platform.log(false, "wear skin: " + e);
			}
		}, "arctic-skins");
		t.setDaemon(true);
		t.start();
	}

	private List<Skin> parse(JsonElement reply) throws Exception {
		List<Skin> out = new ArrayList<Skin>();
		if (reply == null || !reply.isJsonArray()) {
			return out;
		}
		for (JsonElement e : reply.getAsJsonArray()) {
			if (!e.isJsonObject() || out.size() >= MAX_SKINS) {
				continue;
			}
			JsonObject o = e.getAsJsonObject();
			String id = Looks.stringField(o, "id");
			String png = Looks.stringField(o, "png");
			if (id == null || png == null) {
				continue;
			}
			byte[] bytes;
			try {
				bytes = Base64.getDecoder().decode(png);
			} catch (IllegalArgumentException bad) {
				continue;
			}
			String texture = sha1(bytes);
			ArcticClient.looks().registerOwn(texture, bytes);
			String name = Looks.stringField(o, "name");
			if (name == null || name.isEmpty()) {
				name = "Skin";
			} else if (name.length() > MAX_NAME) {
				name = name.substring(0, MAX_NAME);
			}
			JsonElement slim = o.get("slim");
			out.add(new Skin(id, name, slim != null && slim.isJsonPrimitive() && slim.getAsBoolean(), texture));
		}
		return Collections.unmodifiableList(out);
	}

	private static String sha1(byte[] bytes) throws Exception {
		StringBuilder hex = new StringBuilder();
		for (byte b : MessageDigest.getInstance("SHA-1").digest(bytes)) {
			hex.append(String.format("%02x", b));
		}
		return hex.toString();
	}
}
