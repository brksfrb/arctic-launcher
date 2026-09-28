package com.arcticlauncher.client.replay;

/** A camera path point: where the camera is at a replay time. */
public final class Keyframe {
	/** Replay time (ms). */
	public int time;
	public double x;
	public double y;
	public double z;
	public float yaw;
	public float pitch;
	public float fov;

	public Keyframe() {}

	public Keyframe(int time, double[] camera) {
		this.time = time;
		this.x = camera[0];
		this.y = camera[1];
		this.z = camera[2];
		this.yaw = (float) camera[3];
		this.pitch = (float) camera[4];
		this.fov = (float) camera[5];
	}

	public Keyframe copy() {
		Keyframe k = new Keyframe();
		k.time = time;
		k.x = x;
		k.y = y;
		k.z = z;
		k.yaw = yaw;
		k.pitch = pitch;
		k.fov = fov;
		return k;
	}
}
