package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.GameKey;
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
	/** Held keys simulated by the self-test (no keyboard there). */
	private boolean simulatedZoom;
	private boolean simulatedLook;

	private final Combat combat = new Combat();
	private final Toggle sprint = new Toggle(GameKey.SPRINT);
	private final Toggle sneak = new Toggle(GameKey.SNEAK);

	public Features(Platform platform, ClientConfig config) {
		this.platform = platform;
		this.config = config;
	}

	/** 20 times a second while in a world, before the game ticks. */
	public void tick(boolean screenOpen) {
		if (platform.hasFeatures()) {
			sprint.tick(config.toggleSprint, screenOpen);
			sneak.tick(config.toggleSneak, screenOpen);
			combat.tick(platform.hurtTime());
		}
		boolean inGame = !screenOpen && platform.hasFeatures();
		zooming = inGame && config.zoomEnabled && (simulatedZoom || down(config.zoomKey));
		boolean wantLook = inGame && config.freelookEnabled && (simulatedLook || down(config.freelookKey));
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

	/** For the self-test: act as if the Zoom/Freelook keys were held. */
	public void simulate(boolean zoom, boolean look) {
		simulatedZoom = zoom;
		simulatedLook = look;
	}

	public Combat combat() {
		return combat;
	}

	/** "Sprinting (toggled)", "Sneaking (toggled)", or null. */
	public String movement() {
		if (sneak.on) {
			return "Sneaking (toggled)";
		}
		return sprint.on ? "Sprinting (toggled)" : null;
	}

	/** Press the key once to keep a control held; press again to let go. */
	private final class Toggle {
		private final GameKey key;
		private boolean wasDown;
		boolean on;

		Toggle(GameKey key) {
			this.key = key;
		}

		void tick(boolean enabled, boolean screenOpen) {
			if (!enabled) {
				if (on) {
					on = false;
					platform.setKeyDown(key, platform.physicalKeyDown(key));
				}
				return;
			}
			boolean down = !screenOpen && platform.physicalKeyDown(key);
			if (down && !wasDown) {
				on = !on;
			}
			wasDown = down;
			platform.setKeyDown(key, on || down);
		}
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
