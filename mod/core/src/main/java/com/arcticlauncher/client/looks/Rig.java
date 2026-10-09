package com.arcticlauncher.client.looks;

/**
 * The expressive player rig, for emotes that have a {@code torso} or {@code root} bone (the older
 * six-part emotes keep posing each part on its own).
 *
 * <ul>
 * <li>{@code root}: the whole player, drawn only (the entity, its hitbox and the camera stay put):
 * rotation [0, yaw, 0] about the vertical axis and position [sideways, up, forward] in pixels.</li>
 * <li>{@code torso}: bends at the waist and carries the head and both arms with it.</li>
 * <li>{@code head}, {@code rightArm}, {@code leftArm}: rotations inside the torso.</li>
 * <li>{@code rightLeg}, {@code leftLeg}: rotations at the hips, under the root.</li>
 * </ul>
 *
 * Rotations are degrees in the model part's own order (z, then y, then x), as for the six-part
 * emotes; positions are pixels with y up, like Blockbench. Parts are in the game's model space
 * (y down, the player facing -z).
 */
public final class Rig {
	public static final int HEAD = 0;
	public static final int BODY = 1;
	public static final int RIGHT_ARM = 2;
	public static final int LEFT_ARM = 3;
	public static final int RIGHT_LEG = 4;
	public static final int LEFT_LEG = 5;
	public static final int PARTS = 6;

	private static final String[] BONES = {"head", null, "rightArm", "leftArm", "rightLeg", "leftLeg"};
	private static final boolean[] ON_TORSO = {true, true, true, true, false, false};
	private static final float DEG = (float) (Math.PI / 180);
	/** From the body's pivot (the neck) down to the waist, where the torso bends. */
	private static final float WAIST = 12f;

	private Rig() {
	}

	/** Whether {@code animation} uses this rig. */
	public static boolean expressive(Animation animation) {
		return animation.hasBone("torso") || animation.hasBone("root");
	}

	/**
	 * Pose the six parts at {@code t} seconds. {@code pivots} and {@code rotations} (radians) come in
	 * as the game left them and are replaced; a part without its own track keeps its rotation.
	 */
	public static void pose(Animation animation, float t, float[][] pivots, float[][] rotations) {
		float[] rootRot = orZero(animation.rotation("root", t));
		float[] rootPos = orZero(animation.position("root", t));
		float[] root = euler(0, rootRot[1] * DEG, 0);
		float[] torso = euler(orZero(animation.rotation("torso", t)), DEG);
		float[] waist = {pivots[BODY][0], pivots[BODY][1] + WAIST, pivots[BODY][2]};
		// Positions are y up; the model is y down.
		float[] lift = {rootPos[0], -rootPos[1], rootPos[2]};
		for (int i = 0; i < PARTS; i++) {
			float[] local = i == BODY ? identity() : localRotation(animation, BONES[i], t, rotations[i]);
			float[] pivot = pivots[i].clone();
			float[] offset = BONES[i] == null ? null : animation.position(BONES[i], t);
			if (offset != null) {
				pivot[0] += offset[0];
				pivot[1] -= offset[1];
				pivot[2] += offset[2];
			}
			float[] rotation = local;
			if (ON_TORSO[i]) {
				pivot = add(waist, apply(torso, sub(pivot, waist)));
				rotation = mul(torso, rotation);
			}
			pivots[i] = add(apply(root, pivot), lift);
			rotations[i] = angles(mul(root, rotation));
		}
	}

	private static float[] localRotation(Animation animation, String bone, float t, float[] current) {
		float[] r = animation.rotation(bone, t);
		return r == null ? euler(current, 1f) : euler(r, DEG);
	}

	private static float[] orZero(float[] v) {
		return v == null ? new float[3] : v;
	}

	private static float[] euler(float[] v, float scale) {
		return euler(v[0] * scale, v[1] * scale, v[2] * scale);
	}

	/** Rz(z) * Ry(y) * Rx(x), row-major. */
	static float[] euler(float x, float y, float z) {
		float cx = (float) Math.cos(x);
		float sx = (float) Math.sin(x);
		float cy = (float) Math.cos(y);
		float sy = (float) Math.sin(y);
		float cz = (float) Math.cos(z);
		float sz = (float) Math.sin(z);
		return new float[] {
				cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx,
				sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx,
				-sy, cy * sx, cy * cx};
	}

	/** The x, y, z angles of a rotation made as {@link #euler}. */
	static float[] angles(float[] m) {
		float sy = Math.max(-1f, Math.min(1f, -m[6]));
		float y = (float) Math.asin(sy);
		if (Math.abs(sy) > 0.9999f) {
			// Straight up or down: z and x turn about the same axis; put it all in x.
			return new float[] {(float) Math.atan2(-m[5], m[4]), y, 0f};
		}
		return new float[] {(float) Math.atan2(m[7], m[8]), y, (float) Math.atan2(m[3], m[0])};
	}

	private static float[] identity() {
		return new float[] {1, 0, 0, 0, 1, 0, 0, 0, 1};
	}

	private static float[] mul(float[] a, float[] b) {
		float[] out = new float[9];
		for (int r = 0; r < 3; r++) {
			for (int c = 0; c < 3; c++) {
				out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
			}
		}
		return out;
	}

	private static float[] apply(float[] m, float[] v) {
		return new float[] {
				m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
				m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
				m[6] * v[0] + m[7] * v[1] + m[8] * v[2]};
	}

	private static float[] add(float[] a, float[] b) {
		return new float[] {a[0] + b[0], a[1] + b[1], a[2] + b[2]};
	}

	private static float[] sub(float[] a, float[] b) {
		return new float[] {a[0] - b[0], a[1] - b[1], a[2] - b[2]};
	}
}
