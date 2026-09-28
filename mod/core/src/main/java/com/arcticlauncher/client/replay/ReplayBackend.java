package com.arcticlauncher.client.replay;

import java.nio.ByteBuffer;
import java.util.List;
import java.util.UUID;

/**
 * What watching a replay needs from Minecraft. Each version adapter
 * implements this: it turns recorded bytes back into the game's packets and
 * feeds them to a connection that goes nowhere, so the game shows the
 * recording as if it were happening. Everything else (time, jumping, the
 * camera, the timeline) is the core's. Game thread unless noted.
 */
public interface ReplayBackend {
	/** Sorts recorded packets into {@link Category categories}; used on a background thread. */
	interface Classifier {
		/** The category of {@code packet} (its id, then its body); may put a key in {@code key[0]}. */
		byte classify(ByteBuffer packet, long[] key);
	}

	/** The network protocol this game speaks (a replay must match it); 0 if unknown. */
	int protocol();

	/** A packet sorter for this recording (null if the version can't sort: jumping is slower). */
	Classifier classifier(ReplayData data);

	/**
	 * Join the replay: leave any world and connect the game to it. Packets
	 * then go through {@link #apply}, starting with the login. Returns why it
	 * couldn't, or null.
	 */
	String start(ReplayData data);

	/** Begin again from an empty world (for jumping back): like {@link #start}, without menus. */
	String restart(ReplayData data);

	/** Hand one recorded packet to the game; {@code jumping}: nobody sees it, skip what only shows. */
	void apply(ByteBuffer packet, boolean jumping);

	/** The replay's connection is in the world (logged in, configured, playing). */
	boolean ready();

	/**
	 * A "loading terrain" screen is up (after a jump back rebuilt the world).
	 * It closes on a game tick, so the clock mustn't be stopped meanwhile.
	 */
	default boolean loading() {
		return false;
	}

	/** Leave the replay (back to the title screen). */
	void stop();

	/**
	 * The recording player at this moment (null: not recorded). {@code swing}:
	 * it swung its arm since the last call. {@code firstPerson}: the camera is
	 * its eyes, so hide its body and give the camera its hotbar, hands and health.
	 */
	void showSelf(SelfSample at, boolean swing, UUID uuid, String name, boolean firstPerson);

	/** Hide the game's own HUD (exporting a camera path), or bring it back. */
	void cleanView(boolean clean);

	/**
	 * Put the camera at {x, y, z} looking along yaw/pitch (NaN: keep the
	 * mouse's look), with this field of view (0: the player's setting).
	 * {@code firstPerson}: show hands and hotbar as the recording player saw them.
	 */
	void camera(double x, double y, double z, float yaw, float pitch, float fov, boolean firstPerson);

	/** The camera's look right now {yaw, pitch}. */
	float[] look();

	/** Watch through this entity's eyes (-1: back to the free camera). */
	void follow(int entityId);

	/** Players you can follow: {entity id (Integer), name, x, y, z}. */
	List<Object[]> followable();

	/**
	 * Grab the frame just drawn as RGBA rows ({@code width * height * 4}
	 * bytes) and hand it to {@code done}, possibly a frame or two later.
	 */
	void capture(FrameSink done);

	/** Where finished frames go. */
	interface FrameSink {
		/** {@code bottomUp}: the rows come bottom first (as OpenGL reads them). */
		void frame(int width, int height, ByteBuffer rgba, boolean bottomUp);
	}
}
