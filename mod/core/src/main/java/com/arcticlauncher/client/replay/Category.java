package com.arcticlauncher.client.replay;

/**
 * What a recorded packet does, for jumping through a replay quickly. The
 * version adapter sorts each packet into one of these (see
 * {@link ReplayBackend.Classifier}); {@link SeekPlan} then leaves out every
 * packet whose effect a later one replaces.
 */
public final class Category {
	/** Changes lasting state (tab list, scoreboard, login, configuration): always applied. */
	public static final byte ALWAYS = 0;
	/** Sounds, particles, swings: only while watching, skipped when jumping. */
	public static final byte TRANSIENT = 1;
	/** A whole chunk arrives (key: the chunk). */
	public static final byte CHUNK = 2;
	/** A chunk is dropped (key: the chunk). */
	public static final byte UNLOAD = 3;
	/** Blocks or light change inside a chunk (key: the chunk). */
	public static final byte BLOCK = 4;
	/** Respawn or dimension change: a new, empty world. */
	public static final byte RESPAWN = 5;
	/** An entity appears (key: its id). */
	public static final byte ENTITY_ADD = 6;
	/** An entity's absolute position (key: its id): earlier moves no longer matter. */
	public static final byte ENTITY_POS = 7;
	/** An entity moves relative to where it was (key: its id). */
	public static final byte ENTITY_MOVE = 8;
	/** Anything else about one entity (key: its id): gone with a respawn. */
	public static final byte ENTITY_OTHER = 9;
	/** Never shown in a replay (keep-alives, disconnects, screens that would pop up). */
	public static final byte DROP = 10;

	private Category() {}

	/** A chunk's key from its chunk coordinates. */
	public static long chunk(int x, int z) {
		return ((long) x << 32) | (z & 0xFFFFFFFFL);
	}
}
