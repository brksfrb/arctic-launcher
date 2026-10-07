package com.arcticlauncher.client;

import com.arcticlauncher.client.ui.Page;
import java.io.File;
import java.util.List;
import java.util.UUID;

/**
 * Everything the core needs from Minecraft. Each version adapter implements
 * this; the core never touches game classes.
 */
public interface Platform {
	/** The running Minecraft version, like "26.3". */
	String minecraftVersion();

	File configDir();

	void log(boolean warning, String message);

	// ---- Game state -------------------------------------------------------

	int fps();

	/** Latency to the server in ms, or -1 in singleplayer / menus. */
	int ping();

	/** True while playing (a world is loaded). */
	boolean inWorld();

	/** Player {x, y, z, yaw, pitch}, or null outside a world. */
	double[] position();

	boolean keyDown(GameKey key);

	/** The biome at the player, like "Snowy Plains", or null. */
	String biome();

	/** The server's address, "Singleplayer", or null outside a world. */
	String server();

	/** World time in ticks (day = time / 24000), or -1 outside a world. */
	long dayTime();

	// ---- Keys and camera (features) ------------------------------------------------

	/** Is the key with this Minecraft name ("key.keyboard.c") held? */
	boolean isKeyDown(String key);

	/** A key's label for menus ("C", "Left Alt"). */
	String keyLabel(String key);

	/** The Minecraft name of a key the game reported (a native key code). */
	String keyName(int nativeKey);

	/** Switch to third person (true) or back to the view before (false). */
	void setThirdPerson(boolean on);

	/** Zoom, Freelook, Fullbright and the other game features work on this version. */
	boolean hasFeatures();

	/** The smooth font pack is on. */
	boolean smoothFont();

	/** Switch the smooth font (reloads the game's resources). */
	void setSmoothFont(boolean on);

	/** Is the key bound to a control physically held (ignoring toggles)? */
	boolean physicalKeyDown(GameKey key);

	/** Press or release a control as if its key were held. */
	void setKeyDown(GameKey key, boolean down);

	/** How long ago the player was hurt, in ticks (vanilla hurtTime), 0 if not. */
	int hurtTime();

	// ---- HUD data (items are the game's own stacks, drawn with Gfx.item) ------

	/** Worn armor, head to feet, then the main hand: {stack, durability text or null}. */
	java.util.List<Object[]> armor();

	/** Active effects: {name, time left like "1:23", color (Integer ARGB), icon sprite or null}. */
	java.util.List<Object[]> effects();

	/** The held item and how many of it you carry: {stack, total}, or null. */
	Object[] heldItem();

	/**
	 * Bake a cosmetic (its model, PNG texture and, if it has one, the PNG of
	 * the parts that glow) for drawing, on the render thread; then call
	 * {@code Cosmetics.ready(id)}.
	 */
	default void registerCosmetic(String id, com.arcticlauncher.client.looks.Geometry geometry, byte[] png, byte[] glow) {}

	/**
	 * Bake a sculpted cosmetic (its triangles and embedded textures) for
	 * drawing, on the render thread; then call {@code Cosmetics.ready(id)}.
	 */
	default void registerMesh(String id, com.arcticlauncher.client.looks.MeshModel mesh) {}

	/** The local player is moving (walking, jumping, sneaking): stops emotes. */
	default boolean localMoving() {
		return false;
	}

	/**
	 * The local player's UUID in the current world (on offline-mode servers
	 * the one derived from the name), or null outside a world.
	 */
	default java.util.UUID worldPlayerId() {
		return null;
	}

	/** Run on the game's main thread. */
	default void runOnGameThread(Runnable r) {
		r.run();
	}

	/**
	 * Play as another account from now on (outside a world). Returns why it
	 * couldn't, or null when done.
	 */
	default String switchAccount(String name, java.util.UUID uuid, String accessToken, String xuid, boolean microsoft) {
		return "not supported on this version yet";
	}

	/** Ctrl (Cmd on macOS) is held. */
	default boolean controlDown() {
		return false;
	}

	/** Put text on the clipboard. */
	default void setClipboard(String text) {}

	/** Text on the clipboard ("" if none). */
	default String clipboard() {
		return "";
	}

	/** Typing starts or stops (some versions only deliver text while it's on). */
	default void textInput(Object owner, boolean on) {}

	/**
	 * The proxy changed (null = off): route new server connections through
	 * it and answer its login prompts.
	 */
	default void proxyChanged(com.arcticlauncher.client.config.ProxyConfig proxy) {}

	/** A stack of the item with this id (like "minecraft:arrow") for editor previews, or null. */
	default Object sampleItem(String id) {
		return null;
	}

	/** The HUD icon sprite of the effect with this id (like "minecraft:speed"), or null. */
	default Object effectSprite(String id) {
		return null;
	}

	/** The creature you're aiming at or just hit: {name, health, max health}, or null. */
	Object[] target();

	/** The vanilla HUD is hidden (F1) or covered by the debug screen. */
	boolean hudHidden();

	// ---- Screens ----------------------------------------------------------

	/** Show a core page, returning to the current screen when it closes. */
	void openPage(Page page);

	/** Close the current page, back to where it was opened from. */
	void closePage();

	/** Open a vanilla screen or do a vanilla action from the title menu. */
	void action(MenuAction action);

	// ---- Looks ------------------------------------------------------------

	UUID playerId();

	String playerName();

	/**
	 * Make a downloaded PNG drawable as {@code "look:" + hash}. Called off
	 * the render thread; call {@code ArcticClient.looks().textureReady(hash,
	 * frames)} once it's registered. A {@code cape} whose image stacks
	 * several 2:1 frames (see {@code Looks.capeFrames}) is registered as
	 * {@code hash + "/" + i} per frame instead.
	 */
	void registerTexture(String hash, byte[] png, boolean cape);

	/** Mojang session join, to prove who we are to the Arctic server. */
	void joinServer(String serverId) throws Exception;

	/** What you're aiming at: 0 nothing, 1 a player, 2 a hostile mob, 3 another creature. */
	default int aimKind() {
		return 0;
	}

	/**
	 * Whether this version can talk to Simple Voice Chat servers (and the
	 * Simple Voice Chat mod itself isn't installed).
	 */
	default boolean canSimpleVoiceChat() {
		return false;
	}

	/** Ask the server for a Simple Voice Chat secret (it answers through {@code VoiceLink}). */
	default void requestSimpleVoiceChat() {}

	/** The current game connection (changes on every join or server switch), or null. */
	default Object connectionKey() {
		return null;
	}

	/** The world you're in: the server address, or "sp:" + the world's name; null in menus. */
	default String worldKey() {
		return null;
	}

	/** The dimension you're in ("overworld", "the_nether", "the_end", …). */
	default String dimension() {
		return "overworld";
	}

	/** This version has the minimap. */
	default boolean minimapWorks() {
		return false;
	}

	/** Worn armor and held items that wear out: {name, left, max, slot}. */
	default java.util.List<Object[]> durability() {
		return java.util.Collections.emptyList();
	}

	/** A short, quiet ping (someone mentioned you). */
	default void mentionSound() {}

	/** The minimap's texture key, redrawn when due; null where there's none. */
	default String minimap() {
		return null;
	}

	/** The game's resource pack folder, or null. */
	default java.io.File resourcePackDir() {
		return null;
	}

	/** Turn on a pack from the resource pack folder (by file name) and reload. */
	default void enableResourcePack(String fileName) {}

	/** Commands take 1.8.9 to 1.12.2's syntax (camelCase game rules, "effect @a clear", replaceitem). */
	default boolean oldCommands() {
		return false;
	}

	/** This version can make a duel world and open it to LAN. */
	default boolean canDuel() {
		return false;
	}

	/** Leave any world and make a flat duel world (with cheats on). */
	default void createDuelWorld() {}

	/** In a world you host yourself (singleplayer, not a server). */
	default boolean inSingleplayerWorld() {
		return false;
	}

	/** Open the world you're in to LAN; true when it's open. */
	default boolean openToLan() {
		return false;
	}

	/** Tint hurt mobs with this color (RGB; 0 = the game's red). */
	default void setHitColor(int rgb) {}

	/** Hit color works on this version. */
	default boolean hitColorWorks() {
		return false;
	}

	/** Motion blur works on this version. */
	default boolean motionBlurWorks() {
		return false;
	}

	/** Item physics works on this version. */
	default boolean itemPhysicsWorks() {
		return false;
	}

	/** The block outline settings work on this version. */
	default boolean outlineTweaks() {
		return false;
	}

	/** The scoreboard switches work on this version. */
	default boolean scoreboardTweaks() {
		return false;
	}

	/** Where Minecraft's own toasts end (GUI pixels from the top), 0 when none show. */
	default int toastsBottom() {
		return 0;
	}

	/** Light where you stand: {block light, sky light}, or null. */
	default int[] light() {
		return null;
	}

	/** How many items whose id contains {@code idPart} you carry, or -1. */
	/**
	 * The arrows you carry, one row per kind (plain, spectral, each tipped
	 * kind): {an item stack to draw, how many (Integer)}. Empty outside a world.
	 */
	default java.util.List<Object[]> arrows() {
		return java.util.Collections.emptyList();
	}

	default int countItems(String idPart) {
		return -1;
	}

	/** Hunger and saturation: {food 0-20, saturation}, or null. */
	default float[] food() {
		return null;
	}

	/** The top resource pack you picked, or null for none. */
	default String resourcePack() {
		return null;
	}

	/** You're dead (the death screen). */
	default boolean dead() {
		return false;
	}

	/** Send a chat message, or a command when it starts with "/". */
	default void sendChat(String text) {}

	/** Players online on the server (the tab list), or -1 when not connected. */
	default int playerCount() {
		return -1;
	}

	/** Entities the world has loaded around you, or -1 outside a world. */
	default int entityCount() {
		return -1;
	}

	/** Other players in the world: {uuid, name, x, y, z}. */
	List<Object[]> otherPlayers();

	/**
	 * Leave the current world (if any) and join a server. False where this
	 * version can't (the player joins from the multiplayer list instead).
	 */
	boolean connectTo(String address);

	/**
	 * The camera this frame: {x, y, z, yaw, pitch, vertical FOV in degrees},
	 * or null outside a world (for drawing things where they are in the world).
	 */
	default double[] camera() {
		return null;
	}

	/**
	 * The resource packs you have: turned-on ones first (top pack first),
	 * then the rest. Arctic's own hidden packs aren't listed. Game thread.
	 */
	default java.util.List<com.arcticlauncher.client.packs.PackInfo> resourcePacks() {
		return java.util.Collections.emptyList();
	}

	/** Turn on exactly these packs (top first) and reload the game's resources. Game thread. */
	default void setResourcePacks(java.util.List<String> enabledTopFirst) {}

	/** Leave the world or server you're in, back to the title screen (game thread). */
	void leaveWorld();

	/** The player's field of view setting, in degrees. */
	default double fovDegrees() {
		return 70;
	}

	/** Whether the game's smooth (cinematic) camera is on. */
	default boolean smoothCamera() {
		return false;
	}

	/** Turn the game's smooth (cinematic) camera on or off. */
	default void smoothCamera(boolean on) {}
}
