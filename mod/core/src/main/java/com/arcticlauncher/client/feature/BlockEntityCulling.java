package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Block entity culling: chests, signs, shulker boxes, banners, heads and the like that are outside the
 * view aren't drawn.
 *
 * <p>The game culls them only by the 16-block section they sit in: a section that is partly on screen
 * draws every chest and sign in it, each through its own renderer (on 1.8.9 that is about twenty GL
 * calls and a light lookup per chest, and a full text layout per sign line, every frame). Each one is
 * tested on its own here, with a box one block larger than its block on every side, so double chests,
 * open lids, standing and wall banners and shulker lids are always inside it: what is skipped can't
 * be on screen, and what is drawn is drawn by the game exactly as before.
 *
 * <p>The adapters decide which block entities are known (only the game's own kinds, never a modded
 * subclass that may draw farther), find the game's view frustum and call {@link #skip}. A shorter draw
 * distance is the player's choice; {@link #GAME_DISTANCE} keeps the game's.
 */
public final class BlockEntityCulling {
	/** The game's own draw distance for chests, signs and the rest (blocks). */
	public static final int GAME_DISTANCE = 64;
	/** The draw distances offered, the game's first. */
	public static final int[] DISTANCES = {GAME_DISTANCE, 48, 32, 24, 16};
	/** How far a block entity's drawing may reach out of its block, in blocks (lids, banners, the other half of a chest). */
	public static final double REACH = 1.0;

	/** The game's view test for a box in world coordinates. */
	public interface View {
		boolean sees(double minX, double minY, double minZ, double maxX, double maxY, double maxZ);
	}

	/** Shader packs draw block entities from the sun as well (a shadow pass with its own view): no culling then. */
	private static volatile boolean shadowPasses;

	/** The frame's view frustum (1.15+, where the block entity renderer isn't given one), or null. */
	private static volatile Object frustum;
	/** Between {@link #begin} and {@link #end}: the world is being drawn. */
	private static volatile boolean inLevel;

	private BlockEntityCulling() {}

	/** A mod that draws the world more than once per frame (shader shadows, portals) is loaded: culling stays off. */
	public static void shadowPasses(boolean present) {
		shadowPasses = present;
	}

	/** Is culling on at all? */
	public static boolean active(ClientConfig c) {
		return c != null && c.blockEntityCulling && !shadowPasses;
	}

	/** The draw distance in blocks (the game's when culling is off or the setting is out of range). */
	public static int distance(ClientConfig c) {
		if (!active(c) || c.blockEntityDistance <= 0 || c.blockEntityDistance >= GAME_DISTANCE) {
			return GAME_DISTANCE;
		}
		return c.blockEntityDistance;
	}

	/**
	 * Should the block entity at block {@code x, y, z} be left out this frame? The camera is at
	 * {@code camX, camY, camZ}; {@code view} is the game's frustum test (null: no frustum known, only
	 * the distance counts).
	 */
	public static boolean skip(ClientConfig c, int x, int y, int z, double camX, double camY, double camZ, View view) {
		if (!active(c)) {
			return false;
		}
		int distance = distance(c);
		if (distance < GAME_DISTANCE) {
			// Measured to the block's middle, as the game measures its own 64 blocks.
			double dx = x + 0.5 - camX;
			double dy = y + 0.5 - camY;
			double dz = z + 0.5 - camZ;
			if (dx * dx + dy * dy + dz * dz > (double) distance * distance) {
				return true;
			}
		}
		return view != null && !view.sees(x - REACH, y - REACH, z - REACH, x + 1 + REACH, y + 1 + REACH, z + 1 + REACH);
	}

	/** The world starts being drawn (1.15+): no frustum is known yet this frame. */
	public static void begin() {
		frustum = null;
		inLevel = true;
	}

	/** The world has been drawn: block entities drawn after this (in menus, previews) are never culled. */
	public static void end() {
		inLevel = false;
		frustum = null;
	}

	/**
	 * The game tested an entity against this frustum. The first one of a frame is the camera's (entities
	 * are tested before block entities are drawn); a later one from some other pass is ignored.
	 */
	public static void seen(Object view) {
		if (inLevel && frustum == null) {
			frustum = view;
		}
	}

	/** The camera's frustum this frame, or null when it isn't known (then nothing is culled by it). */
	public static Object frustum() {
		return inLevel ? frustum : null;
	}
}
