package com.arcticlauncher.client.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/** Arctic Client settings, in {@code config/arctic.json}. */
public final class ClientConfig {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static final int HUD_VERSION = 2;

	/** Show Arctic looks (skins and capes) at all. */
	public boolean showCosmetics = true;
	/** Players whose Arctic look you chose to hide (UUID strings). */
	public Set<String> hiddenPlayers = new HashSet<String>();
	/** Menu style id (see {@code Style}). */
	public String style = "arctic";
	/** When the launcher's style choice was last applied (its timestamp). */
	public long styleFromLauncher;
	/** HUD widget id → placement. */
	public Map<String, HudSlot> hud = new LinkedHashMap<String, HudSlot>();
	/** Layout format of {@link #hud}; older layouts are reset. */
	public int hudVersion = HUD_VERSION;
	/** Features: Fullbright (on/off), and the keys (Minecraft key names). */
	public boolean fullbright;
	public boolean zoomEnabled = true;
	public boolean freelookEnabled = true;
	public String zoomKey = "key.keyboard.c";
	public String freelookKey = "key.keyboard.left.alt";
	public String fullbrightKey = "key.keyboard.unknown";
	/** Press sprint/sneak once to keep sprinting/sneaking. */
	public boolean toggleSprint;
	public boolean toggleSneak;
	/** View: chat timestamps, stacked repeats, lower fire, no rain. */
	public boolean chatTimestamps;
	public boolean chatStack = true;
	public boolean lowFire;
	public boolean clearWeather;
	public CrosshairConfig crosshair = new CrosshairConfig();

	private transient File file;

	public static ClientConfig load(File configDir) {
		File file = new File(configDir, "arctic.json");
		ClientConfig config = null;
		if (file.isFile()) {
			try {
				String json = new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
				config = GSON.fromJson(json, ClientConfig.class);
			} catch (IOException e) {
				config = null;
			} catch (RuntimeException e) {
				config = null;
			}
		}
		if (config == null) {
			config = new ClientConfig();
		}
		config.file = file;
		config.fillDefaults();
		return config;
	}

	private void fillDefaults() {
		if (hiddenPlayers == null) {
			hiddenPlayers = new HashSet<String>();
		}
		if (hud == null) {
			hud = new LinkedHashMap<String, HudSlot>();
		}
		if (style == null) {
			style = "arctic";
		}
		if (hudVersion < HUD_VERSION) {
			hud.clear();
			hudVersion = HUD_VERSION;
		}
		if (zoomKey == null) {
			zoomKey = "key.keyboard.c";
		}
		if (freelookKey == null) {
			freelookKey = "key.keyboard.left.alt";
		}
		if (crosshair == null) {
			crosshair = new CrosshairConfig();
		}
		if (fullbrightKey == null) {
			fullbrightKey = "key.keyboard.unknown";
		}
	}

	/** Save; returns an error message, or null. */
	public String save() {
		try {
			File dir = file.getParentFile();
			if (dir != null && !dir.isDirectory() && !dir.mkdirs()) {
				return "could not create " + dir;
			}
			Files.write(file.toPath(), GSON.toJson(this).getBytes(StandardCharsets.UTF_8));
			return null;
		} catch (IOException e) {
			return e.toString();
		}
	}
}
