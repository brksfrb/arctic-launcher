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
	/** Animated capes stay on their first frame on your screen (everyone's, not just yours). */
	public boolean reduceCapeMotion;
	/** Ids of animated preset capes you turned "Animate" off for (their still image is worn). */
	public Set<String> capeStill = new HashSet<String>();
	/** Players whose Arctic look you chose to hide (UUID strings). */
	public Set<String> hiddenPlayers = new HashSet<String>();
	/** Menu style id (see {@code Style}). */
	public String style = "arctic";
	/** When the launcher's style choice was last applied (its timestamp). */
	public long styleFromLauncher;
	/** HUD widget id → placement. */
	public Map<String, HudSlot> hud = new LinkedHashMap<String, HudSlot>();
	/** Bumped whenever {@link #hud} is replaced or cleared (the HUD keeps its slots looked up until then). Not saved. */
	public transient int hudEpoch;

	/** The HUD's slots were cleared or replaced. */
	public void hudChanged() {
		hudEpoch++;
	}
	/** Layout format of {@link #hud}; older layouts are reset. */
	public int hudVersion = HUD_VERSION;
	/** Features: Fullbright (on/off), and the keys (Minecraft key names). */
	public boolean fullbright;
	/** With Fullbright: the client's light engine does no work (faster). Takes effect when you rejoin a world. */
	public boolean fullbrightSkipLight;
	public boolean zoomEnabled = true;
	public boolean freelookEnabled = true;
	public String zoomKey = "key.keyboard.c";
	/** OptiFine-style smooth (cinematic) camera while zoomed. */
	public boolean zoomSmoothCamera = true;
	/** How fast the mouse turns while zoomed, in percent of the automatic amount (50, 75, 100, 150). */
	public int zoomSensitivity = 100;
	/** The F3 screen with colored labels and values (see DebugStyle). */
	public boolean debugStyle;
	/** Groups of F3 lines left out (a bit for each of DebugStyle.GROUPS). */
	public int debugHidden;
	/** The F3 screen text size in percent (100 = the game's). */
	public int debugScale = 100;
	/** Colors (indexes of DebugStyle.COLORS) for the F3 screen's labels and values. */
	public int debugLabelColor = 2;
	public int debugValueColor = 0;
	/** How the zoom moves in and out: an index into {@link #ZOOM_CURVES}. */
	public int zoomAnimation = 2;
	/** How long zooming in or out takes, in milliseconds (0: at once). */
	public int zoomDurationMs = 250;
	/** The scroll wheel eases to its new zoom level instead of jumping. */
	public boolean zoomSmoothScroll = true;
	public static final String[] ZOOM_CURVES = {"Instant", "Linear", "Ease out", "Ease in and out", "Snappy"};
	/** The Compass widget's middle tick (ARGB; 0 = the menu's accent color). */
	public int compassPointer;
	public String freelookKey = "key.keyboard.left.alt";
	/** Snaplook: hold the key to see yourself from the front (the player keeps looking where they were). */
	public boolean snaplookEnabled;
	public String snaplookKey = "key.keyboard.unknown";
	public String fullbrightKey = "key.keyboard.unknown";
	/** Opens the emote wheel. */
	public String emoteKey = "key.keyboard.b";
	/** The emote wheel stays open while its key is held (let go to play); off: press to open, click to play. */
	public boolean emoteWheelHold = true;
	/** The camera goes to third person while your own emote plays, so you see it. */
	public boolean emoteThirdPerson = true;
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
	/** 1.7 animations (1.8.9): the sword swings while you block; food and bows swing too; the bow stretches the 1.7 way; sneaking is smooth. */
	public boolean oldBlockHit;
	public boolean oldUseSwing;
	public boolean oldBow;
	public boolean oldSneak;
	/** 1.7 item position: how flat items sit in the hand, in first and in third person (1.8.9). */
	public boolean oldItemPosition;
	public boolean oldThirdPerson;
	public boolean clearWeather;
	/** Motion blur, and how strong (1 low to 4 max). */
	public boolean motionBlur;
	public int motionBlurStrength = 2;
	/** How strong the blur is, 0 to 100 (the camera blur; the strength above is for the blend used under VulkanMod). */
	public int motionBlurAmount = 35;
	/** Which movement is blurred: 0 turning the view and walking toward or away from what you look at, 1 turning only, 2 moving only. */
	public int motionBlurMode;
	/** Dropped items lie flat on the ground and tumble as they fall. */
	public boolean itemPhysics;
	/** Fancy style: smooth font and smooth rounded shapes. */
	public boolean fancy;
	/** The smooth font keeps its anti-aliased (in-between shade) edges; off = every pixel fully on or off. */
	public boolean fontAntialias;
	/** Mods whose "new" mark has been dealt with (opened or switched). */
	public java.util.List<String> seenMods = new java.util.ArrayList<String>();
	/** A smiley button at the end of the chat line opens a box of symbols. */
	public boolean emojiButton = true;
	/** Your own name tag shows above your head in third person (the game leaves it out). */
	public boolean ownNametag = true;
	/** Players with an OptiFine cape (and none of Arctic's) show it, without OptiFine. */
	public boolean optifineCapes = true;
	/** The ping in milliseconds after each name in the player list. */
	public boolean tabPing = true;
	/** The view does not tilt and shake when you take damage. */
	public boolean noHurtCam;
	/** Dynamic FPS: a lower frame rate limit while the game isn't in use. */
	public boolean dynamicFps = true;
	/** The limit in another window (0 = none). */
	public int dynFpsUnfocused = 15;
	/** The limit after {@link #dynFpsIdleSeconds} with nothing moving (0 = none). */
	public int dynFpsIdle = 30;
	public int dynFpsIdleSeconds = 60;
	/** The limit while a menu is open in the game (0 = none). */
	public int dynFpsMenu = 60;
	/** Digits after the point in the XYZ widget (0 = whole blocks). */
	public int coordDecimals;
	/** Lit TNT shows its seconds left above it. */
	public boolean tntTimer;
	/** Screens (inventory, chat, menus) stay open while standing in a nether portal. */
	public boolean portalScreens = true;
	/** The Arctic badge in front of Arctic players' name tags too (it is always in the player list). */
	public boolean badgeNametags = true;
	/** 1.8.9 - 1.12.2: vertex buffers were switched on once (they're off in Minecraft's defaults). */
	public boolean legacyVboOn;
	/** The pause menu's leave button needs a second click. */
	public boolean confirmLeave = true;
	/** Minimap sizes (GUI pixels) and how many blocks across it can show. */
	public static final int[] MINIMAP_SIZES = {64, 96, 128};
	public static final int[] MINIMAP_ZOOMS = {32, 64, 96, 128};
	/** Minimap: shown size (GUI pixels), blocks across, round, turning with you, other players, coordinates under it. */
	public int minimapSize = 96;
	public int minimapZoom = 64;
	public boolean minimapRound;
	public boolean minimapRotate;
	public boolean minimapPlayers = true;
	public boolean minimapCoords = true;
	/** After a disconnect, join the same server again by itself (after {@link #reconnectSeconds}). */
	public boolean autoReconnect;
	public int reconnectSeconds = 5;
	/** The wait in milliseconds (0.1 s and up); 0 means "use {@link #reconnectSeconds}" from older settings. */
	public int reconnectMillis;
	/** Tries in a row before auto-reconnect gives up. */
	public int reconnectTries = 5;

	/** How long to wait before rejoining, in milliseconds. */
	public int reconnectDelayMillis() {
		return reconnectMillis > 0 ? reconnectMillis : Math.max(100, reconnectSeconds * 1000);
	}
	/** The address last joined with Direct Connection (the field starts with it, as in vanilla). */
	public String lastDirect = "";
	/** Tier tags: players' PvP tiers by their names (Tiers.SOURCES[tierSource]; tierMode "" = their best). */
	public boolean tierTags = true;
	public int tierSource;
	public String tierMode = "";
	public boolean tierNametags = true;
	public boolean tierTab = true;
	/** Other players' ping in front of their name tag ("48ms | Steve"). */
	public boolean pingNametags;
	/** Your own row in the player list pulses so it's easy to find. */
	public boolean tabGlowSelf;
	/** Playing with a game controller. */
	public com.arcticlauncher.client.controller.Controller.Settings controller = new com.arcticlauncher.client.controller.Controller.Settings();
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
	/** The last Quickplay command, for Play again. */
	public String quickplayLast = "";
	/** Auto Text: answers sent when a chat line contains a trigger. */
	public boolean autoReply;
	/** Chat Mute: players whose lines never show, and words that hide a line (comma-separated). */
	public String chatMuted = "";
	/** Timestamp spelling: 12-hour clock, AM/PM, seconds, [brackets], bold, italic. */
	public boolean chatStamp12h;
	public boolean chatStampAmPm;
	public boolean chatStampSeconds;
	public boolean chatStampBrackets = true;
	public boolean chatStampBold;
	public boolean chatStampItalic;
	/** Keep the chat lines when you disconnect or change server. */
	public boolean chatKeep;
	/** Hide the chat lines on screen (messages still arrive, the history keeps them). */
	public boolean chatOff;
	/** Hide titles: the big title and subtitle never show. */
	public boolean hideTitles;
	/** Sound Changer: single sounds quieter or louder. */
	public boolean soundChanger;
	public java.util.List<SoundRule> soundRules = new java.util.ArrayList<SoundRule>();
	/** Stacking: only lines within this many seconds of each other, and not blank lines or dividers. */
	public boolean chatStackTimed;
	public int chatStackSeconds = 10;
	public boolean chatStackIgnoreBlank;
	public boolean chatStackIgnoreBreak;
	public String chatHiddenWords = "";
	/** The Items widget's list: item id (or part of it), optionally ":Label", separated by commas. */
	public String countedItems = "totem_of_undying:Totems, ender_pearl:Pearls, golden_apple:Gapples";
	public java.util.List<AutoReply> autoReplies = new java.util.ArrayList<AutoReply>();
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
	/** Damage Tint: the screen takes this color for a moment when you're hurt. */
	public boolean damageTint;
	public int damageTintColor = 0xE11D48;
	/** 0.05 to 1: how strong the tint is right after the hit. */
	public double damageTintStrength = 0.4;
	/** Time of day on your screen (ticks, 6000 = noon), or -1 for the server's. */
	public int timeLock = -1;
	/**
	 * What those were set to when they were last switched off (0 / -1: never),
	 * so switching them back on brings back the player's choice.
	 */
	public int outlineColorOff;
	public float outlineWidthOff;
	public int hitColorOff;
	public int timeLockOff = -1;
	/** A pop-up when armor or the held tool is about to break. */
	public boolean durabilityWarning = true;
	/** Combat feedback: a marker where you aim when you hit, sounds for hits and kills, and a low-health warning. */
	public boolean hitMarker;
	public int hitMarkerColor = 0xFFFFFFFF;
	/** Sound for a hit / a kill: 0 off, 1 ding, 2 orb, 3 level-up. */
	public int hitSound;
	public int killSound;
	public boolean lowHealthWarning;
	/** The warning starts at this many hearts or fewer. */
	public int lowHealthHearts = 3;
	public boolean lowHealthBeep = true;
	/** Locked inventory slots (see SlotLocks): off by default; the key is held while clicking a slot. */
	public boolean slotLocks;
	public String slotLockKey = "key.keyboard.left.alt";
	public int[] lockedSlots = new int[0];
	/** Ctrl+F in a chest or the inventory searches its items by name (see InventorySearch). */
	public boolean containerSearch;
	/** Shulker Peek: hover a shulker box to see inside it. */
	public boolean shulkerPeek;
	/** How Shulker Peek shows: 0 while hovering, 1 while Shift is held (see ContainerPeek). */
	public int shulkerPeekMode = 1;
	/** The key that shows the packed preview while held (see ContainerPeek). */
	public String shulkerPeekKey = "key.keyboard.left.shift";
	/** Held with it, the preview shows every slot. */
	public String shulkerPeekFullKey = "key.keyboard.left.alt";
	/** Held Item: where your hands and the item in them sit in first person (see HeldItemTweak). */
	public boolean heldItem;
	public int heldX;
	public int heldY;
	public int heldZ;
	public int heldScale = 100;
	/** Particle Control: keep only this share (percent) of the particles. */
	public boolean particleControl;
	public int particleAmount = 50;
	/** Particle kinds switched off (bit per {@code ParticleKinds.LABELS} entry). */
	public int particleOff;
	/** Ctrl+K opens the command palette (mods, servers, friends). */
	public boolean commandPalette = true;
	/** Saturation (the hidden part of hunger) drawn on the hunger bar. */
	public boolean saturationBar;
	/** Every entity's hitbox box drawn (like F3+B), from one switch. */
	public boolean hitboxes;
	/** Chunk Borders: the chunk edges as lines, like F3+G in one click. */
	public boolean chunkBorders;
	/** Mob Size: creatures other than players drawn at this scale (0.25 to 3). */
	public boolean mobSize;
	/** Fog: stretch (1 to 10 times) or remove the fog. */
	public boolean fog;
	/** Scrollable Tooltips: a tooltip taller than the screen moves with the mouse wheel. */
	public boolean tooltipScroll;
	/** Glint Colorizer: the enchant shimmer color (0 = the game's purple). */
	public int glintColor;
	public int glintColorOff;
	public boolean fogOff;
	public double fogScale = 3.0;
	public double mobSizeFactor = 1.5;
	/** A pop-up with your coordinates when you die. */
	public boolean deathNotice = true;
	/** New screenshots go straight onto the clipboard. */
	public boolean copyScreenshots;
	/** The HUD style preset last applied to every widget (see HudStyles). */
	public String hudStyle = "clean";
	/** Waypoints drawn where they are in the world (not just on the Compass). */
	public boolean waypointsInWorld = true;
	/** Legacy setting, retained when reading old configs; never starts playback anymore. */
	public boolean replayAutoExport;
	/** Encode the game framebuffer while playing, alongside the independent 3D recording. */
	/** The whole replay system: instant clips, recordings and gameplay video. Applies from the next world join. */
	public boolean replayEnabled = true;
	public boolean replayVideoEnabled = true;
	public int replayVideoHeight = 720;
	public int replayVideoFps = 30;
	/** Saves only the recent rolling window, independently of manual recording. */
	public String replayKey = "key.keyboard.f9";
	public String replayRecordKey = "key.keyboard.f10";
	/** How far back instant 3D and video clips reach (seconds). */
	public int clipSeconds = 30;
	/** Chat lines the game keeps to scroll back through (the game's own is 100). */
	public int chatHistory = 10_000;
	/** Type ":heart:" and it sends the symbol. */
	public boolean chatEmoji = true;
	/** Saved chat filters (the History tab and the filtered-chat HUD widget). */
	public java.util.List<ChatFilter> chatFilters = new java.util.ArrayList<ChatFilter>();
	/** The filter the filtered-chat widget shows (a name from {@link #chatFilters}). */
	public String chatFeedFilter = "";

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
			hudChanged();
			hudVersion = HUD_VERSION;
		}
		if (zoomKey == null) {
			zoomKey = "key.keyboard.c";
		}
		debugLabelColor = Math.max(0, Math.min(7, debugLabelColor));
		debugValueColor = Math.max(0, Math.min(7, debugValueColor));
		zoomAnimation = Math.max(0, Math.min(ZOOM_CURVES.length - 1, zoomAnimation));
		zoomDurationMs = Math.max(0, Math.min(1500, zoomDurationMs));
		if (snaplookKey == null) {
			snaplookKey = "key.keyboard.unknown";
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
		if (countedItems == null) {
			countedItems = "totem_of_undying:Totems, ender_pearl:Pearls, golden_apple:Gapples";
		}
		if (chatMuted == null) {
			chatMuted = "";
		}
		if (chatHiddenWords == null) {
			chatHiddenWords = "";
		}
		if (soundRules == null) {
			soundRules = new java.util.ArrayList<SoundRule>();
		}
		if (autoReplies == null) {
			autoReplies = new java.util.ArrayList<AutoReply>();
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
		chatHistory = Math.max(100, Math.min(100_000, chatHistory));
		if (chatFilters == null) {
			chatFilters = new java.util.ArrayList<ChatFilter>();
		}
		if (chatFeedFilter == null) {
			chatFeedFilter = "";
		}
		if (replayKey == null) {
			replayKey = "key.keyboard.f9";
		}
		if (replayRecordKey == null) {
			replayRecordKey = "key.keyboard.f10";
		}
		if (clipSeconds <= 0) {
			clipSeconds = 30;
		}
		clipSeconds = Math.min(120, clipSeconds);
		replayVideoHeight = replayVideoHeight >= 1080 ? 1080 : 720;
		replayVideoFps = replayVideoFps >= 60 ? 60 : 30;
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
