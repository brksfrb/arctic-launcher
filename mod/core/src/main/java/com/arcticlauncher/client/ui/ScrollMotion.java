package com.arcticlauncher.client.ui;

/** Shared, frame-time based wheel motion in GUI pixels. */
public final class ScrollMotion {
	private static final float GLIDE = 0.018f;
	private float offset;
	private float target;
	private float step;
	private long lastFrame;

	public ScrollMotion(float step) { this.step = step; }
	public void step(float value) { step = value; }
	public float offset() { return offset; }
	public int rounded() { return Math.round(offset); }
	public float target() { return target; }

	public void clamp(float max) {
		max = Math.max(0, max);
		offset = Math.max(0, Math.min(max, offset));
		target = Math.max(0, Math.min(max, target));
	}

	public boolean wheel(double amount, float max) {
		clamp(max);
		float before = target;
		target = Math.max(0, Math.min(Math.max(0, max), target - (float) amount * step));
		if (target != before && lastFrame == 0) lastFrame = System.nanoTime();
		return target != before;
	}

	public void jump(float value, float max) {
		offset = target = value;
		clamp(max);
		lastFrame = 0;
	}

	public void frame(float max) {
		long now = System.nanoTime();
		clamp(max);
		if (lastFrame != 0) advance((now - lastFrame) / 1e9f);
		lastFrame = offset == target ? 0 : now;
	}

	/** Advance by elapsed seconds; also allows deterministic motion tests. */
	public void advance(float dt) {
		dt = Math.max(0, Math.min(0.05f, dt));
		offset += (target - offset) * (1f - (float) Math.exp(-dt / GLIDE));
		if (Math.abs(target - offset) < 0.25f) offset = target;
	}
}
