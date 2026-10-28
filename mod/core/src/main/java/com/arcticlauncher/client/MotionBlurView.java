package com.arcticlauncher.client;

/**
 * What the motion blur shader needs each frame, from how the camera turned since the last one.
 *
 * <p>The blur follows the picture's movement instead of mixing in the last frame (which showed as several
 * copies of the picture when the view turned fast at a low frame rate): every pixel is smeared along the way
 * it moved. A turn of the camera moves each pixel by an amount that doesn't depend on how far away it is, so
 * it is worked out here and in the shader from the two camera angles alone, with no depth buffer.
 *
 * <p>Moving forward or back stretches the picture away from the middle of the screen. How much depends on
 * how far away things are, which isn't known without the depth buffer, so a typical distance is assumed.
 *
 * <p>The smear's length is the movement during a fixed exposure time (the strength setting, 0 to 100, is
 * 0.4 ms of exposure per step), not a share of the frame, so it looks the same at 60 and at 240 FPS.
 */
public final class MotionBlurView {
	/** Milliseconds of exposure per step of the strength setting (0 to 100). */
	private static final float EXPOSURE_MS_PER_STEP = 0.4f;
	/** The most the exposure can be stretched over one frame (a very short frame turns into a long smear otherwise). */
	private static final float MAX_SCALE = 6f;
	/** A turn of more than this many degrees in one frame (a teleport, a flick across the whole view) isn't smeared. */
	private static final float MAX_TURN_DEGREES = 60f;
	/** How far away the things you walk toward are taken to be, in blocks (what sets how much walking stretches the picture). */
	private static final float TYPICAL_DEPTH = 5f;
	/** The most the picture stretches over one exposure (the share of the distance to the middle of the screen). */
	private static final float MAX_RADIAL = 0.2f;
	/** A stretch smaller than this is not drawn. */
	private static final float MIN_RADIAL = 0.0005f;
	/** More than this many blocks in one frame (a teleport) is not a walk. */
	private static final double MAX_STEP = 8.0;
	private static final float MIN_SECONDS = 0.001f;
	private static final float MAX_SECONDS = 0.1f;

	private float lastYaw;
	private float lastPitch;
	private double lastX;
	private double lastY;
	private double lastZ;
	private long lastNanos;
	private boolean hasLast;

	/** Column-major 3x3 matrix: a view-space direction now, to where it was in the last frame. */
	public final float[] toLast = new float[9];
	/** Tangent of half the field of view across and up the screen. */
	public float tanX;
	public float tanY;
	/** How much of a frame's movement the smear covers: exposure / frame time (0: the turn is not smeared). */
	public float scale;
	/** How much the picture stretches from its middle over the exposure, as a share of the distance from the middle. */
	public float radial;

	/** Forget the last frame (blur was off for a while): the next frame isn't compared with it. */
	public void reset() {
		hasLast = false;
	}

	/** The exposure the strength setting (0 to 100) stands for, in milliseconds. */
	public static float exposureMs(int amount) {
		return Math.max(0, Math.min(100, amount)) * EXPOSURE_MS_PER_STEP;
	}

	/** Turning only (no position known). */
	public boolean update(float yaw, float pitch, float fovYDegrees, float aspect, int amount, long nowNanos) {
		return update(yaw, pitch, Double.NaN, 0.0, 0.0, fovYDegrees, aspect, amount, nowNanos, true, false);
	}

	/**
	 * Take this frame's camera: angles in degrees as Minecraft keeps them, and where it is.
	 *
	 * @param turning smear the picture where the camera turned
	 * @param forward stretch the picture where the camera moved toward or away from what it looks at
	 * @return false when there is nothing to smear this frame (the first one, a jump, no movement)
	 */
	public boolean update(float yaw, float pitch, double x, double y, double z, float fovYDegrees, float aspect, int amount,
			long nowNanos, boolean turning, boolean forward) {
		boolean usable = hasLast;
		float lastYaw = this.lastYaw;
		float lastPitch = this.lastPitch;
		double lastX = this.lastX;
		double lastY = this.lastY;
		double lastZ = this.lastZ;
		long before = lastNanos;
		this.lastYaw = yaw;
		this.lastPitch = pitch;
		this.lastX = x;
		this.lastY = y;
		this.lastZ = z;
		lastNanos = nowNanos;
		hasLast = true;
		scale = 0f;
		radial = 0f;
		if (!usable) {
			return false;
		}
		float seconds = Math.max(MIN_SECONDS, Math.min(MAX_SECONDS, (nowNanos - before) / 1e9f));
		float exposure = exposureMs(amount) / 1000f;
		if (exposure <= 0f) {
			return false;
		}
		tanY = (float) Math.tan(Math.toRadians(fovYDegrees) / 2.0);
		tanX = tanY * aspect;
		boolean any = false;
		float dYaw = wrap(yaw - lastYaw);
		float dPitch = pitch - lastPitch;
		if (turning && Math.abs(dYaw) <= MAX_TURN_DEGREES && Math.abs(dPitch) <= MAX_TURN_DEGREES && (dYaw != 0f || dPitch != 0f)) {
			scale = Math.min(MAX_SCALE, exposure / seconds);
			float[][] now = view(yaw, pitch);
			float[][] last = view(lastYaw, lastPitch);
			// A direction now is a direction before = V_last * V_now^T (the views are rotations: inverse = transpose).
			float[][] m = multiply(last, transpose(now));
			for (int column = 0; column < 3; column++) {
				for (int row = 0; row < 3; row++) {
					toLast[column * 3 + row] = m[row][column];
				}
			}
			any = true;
		} else {
			java.util.Arrays.fill(toLast, 0f);
			toLast[0] = 1f;
			toLast[4] = 1f;
			toLast[8] = 1f;
		}
		if (forward && !Double.isNaN(x) && !Double.isNaN(lastX)) {
			double dx = x - lastX;
			double dy = y - lastY;
			double dz = z - lastZ;
			if (Math.sqrt(dx * dx + dy * dy + dz * dz) <= MAX_STEP) {
				double yawRad = Math.toRadians(yaw);
				double pitchRad = Math.toRadians(pitch);
				// The way the camera looks, as Minecraft turns its angles into a direction.
				double fx = -Math.sin(yawRad) * Math.cos(pitchRad);
				double fy = -Math.sin(pitchRad);
				double fz = Math.cos(yawRad) * Math.cos(pitchRad);
				double toward = Math.abs(dx * fx + dy * fy + dz * fz) / seconds;
				radial = (float) Math.min(MAX_RADIAL, toward * exposure / TYPICAL_DEPTH);
				any |= radial > MIN_RADIAL;
			}
		}
		return any;
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
