package com.arcticlauncher.client.replay;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;

/**
 * The camera gliding through keyframes: a Catmull-Rom curve through the
 * positions (so it passes each keyframe exactly, without corners), turning
 * the short way round between angles.
 */
public final class CameraPath {
	private final List<Keyframe> keys;

	public CameraPath(List<Keyframe> keyframes) {
		this.keys = new ArrayList<Keyframe>();
		for (Keyframe k : keyframes) {
			this.keys.add(k.copy());
		}
		Collections.sort(this.keys, new Comparator<Keyframe>() {
			@Override
			public int compare(Keyframe a, Keyframe b) {
				return Integer.compare(a.time, b.time);
			}
		});
		unwrapYaw(this.keys);
	}

	public boolean usable() {
		return keys.size() >= 2;
	}

	public int start() {
		return keys.isEmpty() ? 0 : keys.get(0).time;
	}

	public int end() {
		return keys.isEmpty() ? 0 : keys.get(keys.size() - 1).time;
	}

	/** Camera {x, y, z, yaw, pitch, fov} at replay time {@code t} (ms). */
	public double[] at(double t) {
		int n = keys.size();
		if (n == 0) {
			return null;
		}
		if (n == 1 || t <= keys.get(0).time) {
			return of(keys.get(0));
		}
		if (t >= keys.get(n - 1).time) {
			return of(keys.get(n - 1));
		}
		int i = 0;
		while (i < n - 2 && keys.get(i + 1).time <= t) {
			i++;
		}
		Keyframe p1 = keys.get(i);
		Keyframe p2 = keys.get(i + 1);
		Keyframe p0 = i > 0 ? keys.get(i - 1) : p1;
		Keyframe p3 = i + 2 < n ? keys.get(i + 2) : p2;
		double span = Math.max(1, p2.time - p1.time);
		double u = (t - p1.time) / span;
		return new double[] {
				spline(p0.x, p1.x, p2.x, p3.x, u),
				spline(p0.y, p1.y, p2.y, p3.y, u),
				spline(p0.z, p1.z, p2.z, p3.z, u),
				spline(p0.yaw, p1.yaw, p2.yaw, p3.yaw, u),
				clampPitch(spline(p0.pitch, p1.pitch, p2.pitch, p3.pitch, u)),
				p1.fov + (p2.fov - p1.fov) * u,
		};
	}

	private static double[] of(Keyframe k) {
		return new double[] {k.x, k.y, k.z, k.yaw, k.pitch, k.fov};
	}

	/** Uniform Catmull-Rom between p1 and p2. */
	static double spline(double p0, double p1, double p2, double p3, double u) {
		double u2 = u * u;
		double u3 = u2 * u;
		return 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u2 + (-p0 + 3 * p1 - 3 * p2 + p3) * u3);
	}

	private static double clampPitch(double pitch) {
		return Math.max(-90, Math.min(90, pitch));
	}

	/** Make each yaw the closest turn from the one before (350° then 10° goes via 360°). */
	private static void unwrapYaw(List<Keyframe> keys) {
		for (int i = 1; i < keys.size(); i++) {
			float before = keys.get(i - 1).yaw;
			Keyframe k = keys.get(i);
			float d = k.yaw - before;
			d -= 360f * Math.round(d / 360f);
			k.yaw = before + d;
		}
	}
}
