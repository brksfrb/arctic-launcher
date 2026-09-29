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
	/** Opens the emote wheel. */
	public String emoteKey = "key.keyboard.b";
	/** The emote wheel stays open while its key is held (let go to play); off: press to open, click to play. */
	public boolean emoteWheelHold = true;
	/** Streamer mode: your name, skin and the server are hidden on your screen. */
	public boolean streamerMode;
	/** What your name shows as in streamer mode. */
	public String streamerName = "Streamer";
	/** Switches streamer mode. */
	public String streamerKey = "key.keyboard.unknown";
	/** Drops a waypoint where you stand. */
	public String waypointKey = "key.keyboard.unknown";
	/** Starts, stops and clears the stopwatch widget. */
	public String stopwatchKey = "key.keyboard.unknown";
	/** Hold to talk (voice chat in push-to-talk mode). */
	public String voiceKey = "key.keyboard.v";
	/** Press sprint/sneak once to keep sprinting/sneaking. */
	public boolean toggleSprint;
	public boolean toggleSneak;
	/** View: chat timestamps, stacked repeats, lower fire, no rain. */
	public boolean chatTimestamps;
	public boolean chatStack = true;
	/** Chat lines that say your name are marked, with a soft ping. */
	public boolean chatMentions = true;
	public boolean lowFire;
	public boolean clearWeather;
	/** Fancy style: smooth font and smooth rounded shapes. */
	public boolean fancy;
	/** The pause menu's leave button needs a second click. */
	public boolean confirmLeave = true;
	/** SOCKS5 proxy for server connections and Arctic's web requests. */
	public ProxyConfig proxy = new ProxyConfig();
	/** When the launcher's proxy choice was last applied (its timestamp). */
	public long proxyFromLauncher;
	public CrosshairConfig crosshair = new CrosshairConfig();
	/** Keys that send a message or command. */
	public java.util.List<QuickMessage> quickMessages = new java.util.ArrayList<QuickMessage>();
	/** Say this when a minigame ends (Auto GG). */
	public boolean autoGg;
	public String autoGgMessage = "gg";
	/** {x} {y} {z}... also work in chat you type. */
	public boolean chatPlaceholders = true;
	/** Scoreboard: its red numbers, its dark background, or none of it. */
	public boolean scoreboardNumbers = true;
	public boolean scoreboardBackground = true;
	public boolean scoreboardHidden;
	/** Block outline color (ARGB, 0 = the game's) and thickness (1 = the game's). */
	public int outlineColor;
	public float outlineWidth = 1f;
	/** The tint on hurt mobs and players (RGB, 0 = the game's red). */
	public int hitColor;
	/** Time of day on your screen (ticks, 6000 = noon), or -1 for the server's. */
	public int timeLock = -1;
	/** A pop-up when armor or the held tool is about to break. */
	public boolean durabilityWarning = true;
	/** A pop-up with your coordinates when you die. */
	public boolean deathNotice = true;
	/** New screenshots go straight onto the clipboard. */
	public boolean copyScreenshots;
	/** The HUD style preset last applied to every widget (see HudStyles). */
	public String hudStyle = "clean";
	/** Waypoints drawn where they are in the world (not just on the Compass). */
	public boolean waypointsInWorld = true;
	/** Every session records quietly; the replay key keeps it (otherwise it's deleted on leaving). */
	public boolean replayRecording = true;
	/** Saves the session so far as a replay, with a moment marked at the press. */
	public String replayKey = "key.keyboard.f9";
	/** How far back a saved moment reaches (seconds), for 2D clips. */
	public int clipSeconds = 30;

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
		if (waypointKey == null) {
			waypointKey = "key.keyboard.unknown";
		}
		if (stopwatchKey == null) {
			stopwatchKey = "key.keyboard.unknown";
		}
		if (autoGgMessage == null) {
			autoGgMessage = "gg";
		}
		if (quickMessages == null) {
			quickMessages = new java.util.ArrayList<QuickMessage>();
		}
		if (fullbrightKey == null) {
			fullbrightKey = "key.keyboard.unknown";
		}
		if (emoteKey == null) {
			emoteKey = "key.keyboard.b";
		}
		if (streamerKey == null) {
			streamerKey = "key.keyboard.unknown";
		}
		if (voiceKey == null) {
			voiceKey = "key.keyboard.v";
		}
		if (replayKey == null) {
			replayKey = "key.keyboard.f9";
		}
		if (clipSeconds <= 0) {
			clipSeconds = 30;
		}
		if (proxy == null) {
			proxy = new ProxyConfig();
		}
		proxy.fillDefaults();
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
