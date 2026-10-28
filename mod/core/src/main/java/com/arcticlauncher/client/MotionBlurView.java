package com.arcticlauncher.client;

/**
 * What the motion blur shader needs each frame, from how the camera turned since the last one.
 *
 * <p>The blur follows the picture's movement instead of mixing in the last frame (which showed as several
 * copies of the picture when the view turned fast at a low frame rate): every pixel is smeared along the way
 * it moved. A turn of the camera moves each pixel by an amount that doesn't depend on how far away it is, so
 * it is worked out here and in the shader from the two camera angles alone, with no depth buffer.
 *
 * <p>The smear's length is the movement during a fixed exposure time (an "exposure" in milliseconds per
 * strength), not a share of the frame, so it looks the same at 60 and at 240 FPS.
 */
public final class MotionBlurView {
	/** Exposure in ms for strength 1 (Low) to 4 (Max). */
	public static final float[] EXPOSURE_MS = {8f, 14f, 22f, 32f};
	/** The most the exposure can be stretched over one frame (a very short frame turns into a long smear otherwise). */
	private static final float MAX_SCALE = 6f;
	/** A turn of more than this many degrees in one frame (a teleport, a flick across the whole view) isn't smeared. */
	private static final float MAX_TURN_DEGREES = 60f;
	private static final float MIN_SECONDS = 0.001f;
	private static final float MAX_SECONDS = 0.1f;

	private float lastYaw;
	private float lastPitch;
	private long lastNanos;
	private boolean hasLast;

	/** Column-major 3x3 matrix: a view-space direction now, to where it was in the last frame. */
	public final float[] toLast = new float[9];
	/** Tangent of half the field of view across and up the screen. */
	public float tanX;
	public float tanY;
	/** How much of a frame's movement the smear covers: exposure / frame time. */
	public float scale;

	/** Forget the last frame (blur was off for a while): the next frame isn't compared with it. */
	public void reset() {
		hasLast = false;
	}

	/**
	 * Take this frame's camera, in degrees as Minecraft keeps them.
	 *
	 * @return false when there is nothing to smear this frame (the first one, a jump, no turn)
	 */
	public boolean update(float yaw, float pitch, float fovYDegrees, float aspect, int strength, long nowNanos) {
		boolean usable = hasLast;
		float lastYaw = this.lastYaw;
		float lastPitch = this.lastPitch;
		long before = lastNanos;
		this.lastYaw = yaw;
		this.lastPitch = pitch;
		lastNanos = nowNanos;
		hasLast = true;
		if (!usable) {
			return false;
		}
		float dYaw = wrap(yaw - lastYaw);
		float dPitch = pitch - lastPitch;
		if (Math.abs(dYaw) > MAX_TURN_DEGREES || Math.abs(dPitch) > MAX_TURN_DEGREES) {
			return false;
		}
		if (dYaw == 0f && dPitch == 0f) {
			return false;
		}
		float seconds = Math.max(MIN_SECONDS, Math.min(MAX_SECONDS, (nowNanos - before) / 1e9f));
		int level = Math.max(1, Math.min(EXPOSURE_MS.length, strength));
		scale = Math.min(MAX_SCALE, EXPOSURE_MS[level - 1] / 1000f / seconds);
		tanY = (float) Math.tan(Math.toRadians(fovYDegrees) / 2.0);
		tanX = tanY * aspect;
		float[][] now = view(yaw, pitch);
		float[][] last = view(lastYaw, lastPitch);
		// A direction now is a direction before = V_last * V_now^T (the views are rotations: inverse = transpose).
		float[][] m = multiply(last, transpose(now));
		for (int column = 0; column < 3; column++) {
			for (int row = 0; row < 3; row++) {
				toLast[column * 3 + row] = m[row][column];
			}
		}
		return true;
	}

	/** Where a direction seen at {@code (x, y)} (-1 to 1 across the screen) was in the last frame, as such coordinates. */
	public float[] lastPosition(float x, float y) {
		float dx = x * tanX;
		float dy = y * tanY;
		float dz = -1f;
		float px = toLast[0] * dx + toLast[3] * dy + toLast[6] * dz;
		float py = toLast[1] * dx + toLast[4] * dy + toLast[7] * dz;
		float pz = toLast[2] * dx + toLast[5] * dy + toLast[8] * dz;
		return new float[] {px / -pz / tanX, py / -pz / tanY};
	}

	private static float wrap(float degrees) {
		float d = degrees % 360f;
		if (d > 180f) {
			d -= 360f;
		} else if (d < -180f) {
			d += 360f;
		}
		return d;
	}

	/** The game's view rotation: pitch about X, then yaw (+180) about Y, as OpenGL's glRotate multiplies them. */
	private static float[][] view(float yaw, float pitch) {
		return multiply(rotateX(pitch), rotateY(yaw + 180f));
	}

	private static float[][] rotateX(float degrees) {
		double a = Math.toRadians(degrees);
		float c = (float) Math.cos(a);
		float s = (float) Math.sin(a);
		return new float[][] {{1, 0, 0}, {0, c, -s}, {0, s, c}};
	}

	private static float[][] rotateY(float degrees) {
		double a = Math.toRadians(degrees);
		float c = (float) Math.cos(a);
		float s = (float) Math.sin(a);
		return new float[][] {{c, 0, s}, {0, 1, 0}, {-s, 0, c}};
	}

	private static float[][] multiply(float[][] a, float[][] b) {
		float[][] out = new float[3][3];
		for (int i = 0; i < 3; i++) {
			for (int j = 0; j < 3; j++) {
				for (int k = 0; k < 3; k++) {
					out[i][j] += a[i][k] * b[k][j];
				}
			}
		}
		return out;
	}

	private static float[][] transpose(float[][] a) {
		float[][] out = new float[3][3];
		for (int i = 0; i < 3; i++) {
			for (int j = 0; j < 3; j++) {
				out[i][j] = a[j][i];
			}
		}
		return out;
	}
}
