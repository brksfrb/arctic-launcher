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

	/** Zoom, Freelook and Fullbright work on this version. */
	boolean hasFeatures();

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

	/** Other players in the world: {uuid, name}. */
	List<Object[]> otherPlayers();
}
