package com.arcticlauncher.client.replay;

/**
 * The replay's flying camera: moves in real time (paused or at 16× alike),
 * toward where you look, easing in and out. The mouse turns it the way it
 * turns the player; the scroll wheel changes its speed.
 */
public final class FreeCamera {
	static final double MIN_SPEED = 1;
	static final double MAX_SPEED = 120;
	private static final double DEFAULT_SPEED = 10;
	private static final double SCROLL_FACTOR = 1.25;
	/** How quickly it reaches full speed or stops (per second). */
	private static final double EASE = 9;
	private static final double SPRINT = 3;

	double x;
	double y;
	double z;
	private double vx;
	private double vy;
	private double vz;
	private double speed = DEFAULT_SPEED;

	void placeAt(double x, double y, double z) {
		this.x = x;
		this.y = y;
		this.z = z;
		vx = vy = vz = 0;
	}

	/** Speed in blocks a second. */
	double speed() {
		return speed;
	}

	void scroll(double notches) {
		speed = Math.max(MIN_SPEED, Math.min(MAX_SPEED, speed * Math.pow(SCROLL_FACTOR, notches)));
	}

	/**
	 * Move for {@code seconds}: {@code forward}, {@code strafe} (right) and
	 * {@code up} are -1..1; yaw/pitch in Minecraft's degrees.
	 */
	void update(double seconds, double forward, double strafe, double up, boolean sprint, float yaw, float pitch) {
		double yawRad = Math.toRadians(yaw);
		double pitchRad = Math.toRadians(pitch);
		// Minecraft: yaw 0 looks toward +z, 90 toward -x; pitch 90 looks down.
		double lookX = -Math.sin(yawRad) * Math.cos(pitchRad);
		double lookY = -Math.sin(pitchRad);
		double lookZ = Math.cos(yawRad) * Math.cos(pitchRad);
		double rightX = -Math.cos(yawRad);
		double rightZ = -Math.sin(yawRad);
		double tx = lookX * forward + rightX * strafe;
		double ty = lookY * forward + up;
		double tz = lookZ * forward + rightZ * strafe;
		double len = Math.sqrt(tx * tx + ty * ty + tz * tz);
		double s = speed * (sprint ? SPRINT : 1);
		if (len > 1e-6) {
			tx = tx / len * s;
			ty = ty / len * s;
			tz = tz / len * s;
		}
		double k = 1 - Math.exp(-EASE * seconds);
		vx += (tx - vx) * k;
		vy += (ty - vy) * k;
		vz += (tz - vz) * k;
		x += vx * seconds;
		y += vy * seconds;
		z += vz * seconds;
	}
}
