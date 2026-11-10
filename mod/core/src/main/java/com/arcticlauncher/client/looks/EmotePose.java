package com.arcticlauncher.client.looks;

/**
 * The ordinary (non-expressive) emote's move of one model part, on plain numbers: {x, y, z, xRot,
 * yRot, zRot} (pivot in pixels, rotation in radians). Kept apart from the game's classes so it can be tested.
 */
public final class EmotePose {
	private static final float DEG = (float) (Math.PI / 180);

	private EmotePose() {
	}

	/** Rotate {@code part} to the bone's rotation and move it by the bone's offset (both only if animated). */
	public static void pose(Animation animation, String bone, float[] part, float t) {
		float[] r = animation.rotation(bone, t);
		if (r != null) {
			part[3] = r[0] * DEG;
			part[4] = r[1] * DEG;
			part[5] = r[2] * DEG;
		}
		float[] p = animation.position(bone, t);
		if (p != null) {
			part[0] += p[0];
			part[1] -= p[1];
			part[2] += p[2];
		}
	}
}
