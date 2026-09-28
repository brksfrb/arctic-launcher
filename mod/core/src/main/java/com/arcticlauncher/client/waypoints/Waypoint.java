package com.arcticlauncher.client.waypoints;

/** A named spot in one dimension of one world or server. */
public final class Waypoint {
	public String name = "Waypoint";
	public int x;
	public int y;
	public int z;
	/** "overworld", "the_nether", "the_end", … */
	public String dim = "overworld";
	public int color = 0xFF7DD3FC;
	public boolean shown = true;

	/** Blocks from a position, ignoring height (what a compass shows). */
	public double distance(double px, double pz) {
		return Math.hypot(x + 0.5 - px, z + 0.5 - pz);
	}

	/** Minecraft yaw (0 = south, 90 = west) that faces this spot from a position. */
	public double yawFrom(double px, double pz) {
		return Math.toDegrees(Math.atan2(-(x + 0.5 - px), z + 0.5 - pz));
	}
}
