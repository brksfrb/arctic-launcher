package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Zoom (hold), Freelook (hold) and Fullbright (toggle). Adapters ask this
 * for the FOV factor, gamma and camera angles; keys are polled each tick.
 */
public final class Features {
	/** Gamma while Fullbright is on (vanilla's slider tops out at 1). */
	public static final double FULLBRIGHT_GAMMA = 16.0;
	private static final float ZOOM_FACTOR = 0.25f;
	private static final float ZOOM_SPEED = 12f;
	/** Mouse-to-degrees factor, as vanilla turns the player. */
	private static final double TURN_SCALE = 0.15;
	private static final float MAX_PITCH = 90f;
	private static final String UNBOUND = "key.keyboard.unknown";

	private final Platform platform;
	private final ClientConfig config;

	private boolean zooming;
	private float zoom;
	private long lastFrame;

	private boolean freelook;
	private float lookYaw;
	private float lookPitch;

	private boolean fullbrightKeyWasDown;

	public Features(Platform platform, ClientConfig config) {
		this.platform = platform;
		this.config = config;
	}

	/** 20 times a second while in a world: read the held keys. */
	public void tick(boolean screenOpen) {
		boolean inGame = !screenOpen && platform.hasFeatures();
		zooming = inGame && config.zoomEnabled && down(config.zoomKey);
		boolean wantLook = inGame && config.freelookEnabled && down(config.freelookKey);
		if (wantLook != freelook) {
			setFreelook(wantLook);
		}
		boolean fullbrightDown = inGame && down(config.fullbrightKey);
		if (fullbrightDown && !fullbrightKeyWasDown) {
			config.fullbright = !config.fullbright;
			config.save();
		}
		fullbrightKeyWasDown = fullbrightDown;
	}

	private boolean down(String key) {
		return key != null && !UNBOUND.equals(key) && platform.isKeyDown(key);
	}

	private void setFreelook(boolean on) {
		double[] p = platform.position();
		if (on && p == null) {
			return;
		}
		freelook = on;
		if (on) {
			lookYaw = (float) p[3];
			lookPitch = (float) p[4];
		}
		platform.setThirdPerson(on);
	}

	// ---- Zoom ------------------------------------------------------------------

	/** Multiply the field of view by this (eases in and out). */
	public float fovMultiplier() {
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		float target = zooming ? 1f : 0f;
		float step = dt * ZOOM_SPEED * Math.max(0.15f, Math.abs(target - zoom));
		zoom = zoom < target ? Math.min(target, zoom + step) : Math.max(target, zoom - step);
		return 1f - (1f - ZOOM_FACTOR) * zoom;
	}

	/** Slower mouse while zoomed, so aiming stays controllable. */
	public double sensitivityMultiplier() {
		return 1.0 - (1.0 - ZOOM_FACTOR) * zoom;
	}

	// ---- Freelook --------------------------------------------------------------

	public boolean freelook() {
		return freelook;
	}

	/** Mouse movement while freelooking turns the camera, not the player. */
	public boolean turn(double dx, double dy) {
		if (!freelook) {
			return false;
		}
		lookYaw += (float) (dx * TURN_SCALE);
		lookPitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, lookPitch + (float) (dy * TURN_SCALE)));
		return true;
	}

	public float lookYaw() {
		return lookYaw;
	}

	public float lookPitch() {
		return lookPitch;
	}

	// ---- Fullbright ------------------------------------------------------------

	public boolean fullbright() {
		return config.fullbright;
	}
}
