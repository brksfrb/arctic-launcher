package com.arcticlauncher.client.looks;

/**
 * What the cosmetic drawers need from a game version: the pose of the player
 * part being drawn on (turning points and directions into the space the
 * vertices are written in) and a place to write the vertices. One per draw;
 * the result of {@link #position} and {@link #normal} is left in
 * {@link #x}, {@link #y}, {@link #z}.
 */
public abstract class Xform {
	/** Packed light for parts that glow. */
	public static final int FULL_BRIGHT = 0xF000F0;

	public float x;
	public float y;
	public float z;

	/** A point (in blocks, relative to the player part) turned by the pose. */
	public abstract void position(float px, float py, float pz);

	/** A direction turned by the pose's normal matrix. */
	public abstract void normal(float nx, float ny, float nz);

	/** One finished vertex (already in the pose's target space); {@code argb} is the packed tint. */
	public abstract void vertex(float vx, float vy, float vz, int argb, float u, float v, int light, float nx, float ny, float nz);
}
