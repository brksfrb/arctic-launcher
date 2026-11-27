package com.arcticlauncher.client.feature;

/**
 * Vanilla's limit on how far a boat rider's head turns from the boat (1.9 and later), as a
 * reference for Boat look: the camera is free, but the player's own yaw (the one sent to the
 * server) must stay inside this range.
 */
public final class BoatLook {
	/** Degrees either side of the boat's yaw (Boat.clampRotation). */
	public static final float LIMIT = 105f;

	private BoatLook() {
	}

	public static float wrap(float degrees) {
		float d = degrees % 360f;
		if (d >= 180f) {
			d -= 360f;
		} else if (d < -180f) {
			d += 360f;
		}
		return d;
	}

	/** The rider's yaw as vanilla leaves it: within {@link #LIMIT} of the boat's. */
	public static float vanillaYaw(float riderYaw, float boatYaw) {
		float relative = wrap(riderYaw - boatYaw);
		float clamped = Math.max(-LIMIT, Math.min(LIMIT, relative));
		return riderYaw + clamped - relative;
	}

	/** Is this rider yaw one vanilla could send? */
	public static boolean withinVanilla(float riderYaw, float boatYaw) {
		return Math.abs(wrap(riderYaw - boatYaw)) <= LIMIT + 1e-3f;
	}
}
