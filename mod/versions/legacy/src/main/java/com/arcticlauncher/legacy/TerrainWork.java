package com.arcticlauncher.legacy;

/**
 * Terrain meshing limits. The game builds the chunk section the camera is in on the client thread, however
 * much is in it; a section full of glass, leaves or other see-through blocks takes tens of milliseconds, every
 * time a block in it changes. Sections that were slow to build the last time go to the meshing threads instead.
 */
public final class TerrainWork {
	/** A section that took longer than this to build is not built on the client thread again. */
	public static final long SLOW_BUILD_NANOS = Long.getLong("arctic.terrain.syncBuildMs", 4L) * 1_000_000L;
	/**
	 * The game starts the cutout layers at 512 KB and the translucent layer at 1 MB, and grows them by 2 MB with a
	 * warning when a section needs more; these are the sizes after that first growth (in ints, as the game counts).
	 */
	public static final int CUTOUT_INTS = 655_360;
	public static final int TRANSLUCENT_INTS = 786_432;

	private TerrainWork() {}

	/** A chunk section that remembers how long its last build took. */
	public interface Timed {
		long arctic$buildNanos();
	}
}
