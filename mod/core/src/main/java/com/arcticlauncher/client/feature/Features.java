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
	/** Scrolling while zoomed: each notch multiplies the view by this. */
	private static final float SCROLL_STEP = 0.8f;
	private static final float MIN_FACTOR = 0.03f;
	private static final float MAX_FACTOR = 0.6f;
	/** How far zoom goes this time (the wheel changes it; back to normal on release). */
	private float factor = ZOOM_FACTOR;
	/** Mouse-to-degrees factor, as vanilla turns the player. */
	private static final double TURN_SCALE = 0.15;
	private static final float MAX_PITCH = 90f;
	private static final String UNBOUND = "key.keyboard.unknown";

	private final Platform platform;
	private final ClientConfig config;

	private boolean zooming;
	private float zoom;
	/** How far along zooming in (1) or out (0) is, in time: the curve turns it into {@link #zoom}. */
	private float zoomProgress;
	/** The zoom level on screen: eases to {@link #factor} when the wheel moves it. */
	private float shownFactor = ZOOM_FACTOR;
	private static final float SCROLL_EASE = 14f;
	/** The smooth camera setting from before zooming, restored after. */
	private boolean smoothCameraBefore;
	private long lastFrame;

	private boolean freelook;
	/** The camera is in third person for your emote (see {@link #emoteView}). */
	private boolean emoteView;
	/** The server we last said bans Freelook (said once per server). */
	private String toldFreelookBanned;
	private float lookYaw;
	private float lookPitch;

	private boolean fullbrightKeyWasDown;
	private boolean streamerKeyWasDown;
	private boolean emoteKeyWasDown;
	/** Held keys simulated by the self-test (no keyboard there). */
	private boolean simulatedZoom;
	private boolean simulatedLook;

	private final Combat combat = new Combat();
	private final CombatFx combatFx = new CombatFx();
	private final Toggle sprint = new Toggle(GameKey.SPRINT);
	private final Toggle sneak = new Toggle(GameKey.SNEAK);

	public Features(Platform platform, ClientConfig config) {
		this.platform = platform;
		this.config = config;
		combat.onHit(() -> combatFx.hit(config, platform));
	}

	/** 20 times a second while in a world, before the game ticks. */
	public void tick(boolean screenOpen) {
		if (platform.hasFeatures()) {
			sprint.tick(config.toggleSprint, screenOpen);
			sneak.tick(config.toggleSneak, screenOpen);
			combat.tick(platform.hurtTime());
			combatFx.tick(config, platform);
		}
		boolean inGame = !screenOpen && platform.hasFeatures();
		boolean wasZooming = zooming;
		zooming = inGame && config.zoomEnabled && (simulatedZoom || down(config.zoomKey));
		if (zooming != wasZooming) {
			smoothCameraWhileZoomed(zooming);
		}
		boolean wantLook = inGame && config.freelookEnabled && (simulatedLook || down(config.freelookKey));
		if (wantLook && !ServerRules.allowed(ServerRules.FREELOOK, platform.server())) {
			wantLook = false;
			String server = platform.server();
			if (!server.equals(toldFreelookBanned)) {
				toldFreelookBanned = server;
				com.arcticlauncher.client.notice.Notices.post("Freelook is off here", "This server's rules don't allow it");
			}
		}
		if (wantLook != freelook) {
			setFreelook(wantLook);
		}
		emoteView(screenOpen);
		boolean fullbrightDown = inGame && down(config.fullbrightKey);
		if (fullbrightDown && !fullbrightKeyWasDown) {
			config.fullbright = !config.fullbright;
			config.save();
		}
		fullbrightKeyWasDown = fullbrightDown;
		boolean streamerDown = inGame && down(config.streamerKey);
		if (streamerDown && !streamerKeyWasDown) {
			config.streamerMode = !config.streamerMode;
			config.save();
			com.arcticlauncher.client.notice.Notices.post(
					config.streamerMode ? "Streamer mode on" : "Streamer mode off",
					config.streamerMode ? "Your name, skin and the server are hidden on screen." : "");
		}
		streamerKeyWasDown = streamerDown;
		boolean emoteDown = !screenOpen && down(config.emoteKey);
		if (emoteDown && !emoteKeyWasDown) {
			platform.openPage(new com.arcticlauncher.client.menu.EmoteWheel(config.emoteWheelHold));
		}
		emoteKeyWasDown = emoteDown;
	}

	private boolean down(String key) {
		return key != null && !UNBOUND.equals(key) && platform.isKeyDown(key);
	}

	/**
	 * Third person while your own emote plays (once no screen is up, so the
	 * wheel's preview doesn't move the camera), and back when it ends.
	 */
	private void emoteView(boolean screenOpen) {
		boolean playing = config.emoteThirdPerson && platform.hasFeatures() && ownEmotePlaying();
		if (playing && !emoteView && !screenOpen && !freelook) {
			emoteView = true;
			platform.setThirdPerson(true);
		} else if (!playing && emoteView) {
			emoteView = false;
			platform.setThirdPerson(false);
		}
	}

	private boolean ownEmotePlaying() {
		java.util.UUID me = platform.worldPlayerId();
		return me != null && com.arcticlauncher.client.ArcticClient.looks().cosmetics().playingFor(me) != null;
	}

	private void setFreelook(boolean on) {
		double[] p = platform.position();
		if (on && p == null) {
			return;
		}
		if (on && emoteView) {
			// Freelook takes the camera over; the emote's view ends here.
			emoteView = false;
			platform.setThirdPerson(false);
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

	public CombatFx combatFx() {
		return combatFx;
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
		float seconds = config.zoomDurationMs / 1000f;
		if (config.zoomAnimation == 0 || seconds <= 0f) {
			zoomProgress = target;
		} else {
			float step = dt / seconds;
			zoomProgress = zoomProgress < target ? Math.min(target, zoomProgress + step) : Math.max(target, zoomProgress - step);
		}
		zoom = curve(config.zoomAnimation, zoomProgress);
		if (!zooming && zoomProgress == 0f) {
			factor = ZOOM_FACTOR;
			shownFactor = ZOOM_FACTOR;
		} else if (config.zoomSmoothScroll) {
			shownFactor += (factor - shownFactor) * (1f - (float) Math.exp(-dt * SCROLL_EASE));
		} else {
			shownFactor = factor;
		}
		return 1f - (1f - shownFactor) * zoom;
	}

	/** The zoom curve {@code kind} (an index of {@code ClientConfig.ZOOM_CURVES}) at {@code p} (0 to 1). */
	static float curve(int kind, float p) {
		p = Math.max(0f, Math.min(1f, p));
		switch (kind) {
			case 0:
				return p >= 0.5f ? 1f : 0f;
			case 2: {
				float inv = 1f - p;
				return 1f - inv * inv * inv;
			}
			case 3:
				return p < 0.5f ? 4f * p * p * p : 1f - (float) Math.pow(-2f * p + 2f, 3) / 2f;
			case 4:
				return p >= 1f ? 1f : 1f - (float) Math.pow(2.0, -10.0 * p);
			default:
				return p;
		}
	}

	/**
	 * Slower mouse while zoomed, by how much smaller things move on screen:
	 * the ratio of the zoomed and normal views' half-angle tangents (not the
	 * angles themselves, which turns too fast when zoomed far in).
	 */
	public double sensitivityMultiplier() {
		if (zoom <= 0f) {
			return 1.0;
		}
		double fov = Math.toRadians(Math.max(1.0, Math.min(170.0, platform.fovDegrees())));
		double zoomedFov = fov * (1.0 - (1.0 - shownFactor) * zoom);
		double zoomedTurn = Math.tan(zoomedFov / 2) / Math.tan(fov / 2);
		// The player's own setting, faded in with the zoom.
		return zoomedTurn * (1.0 + (config.zoomSensitivity / 100.0 - 1.0) * zoom);
	}

	/** While zoomed, the smooth (cinematic) camera like OptiFine's zoom; the player's own setting comes back after. */
	private void smoothCameraWhileZoomed(boolean on) {
		if (!config.zoomSmoothCamera) {
			return;
		}
		if (on) {
			boolean already = platform.smoothCamera();
			// Already smooth as the zoom starts: another zoom (OptiFine's, on the same key) switched it on
			// for itself. Putting "on" back afterwards left the camera smooth for good: go back to off.
			smoothCameraBefore = false;
			if (!already) {
				platform.smoothCamera(true);
			}
		} else {
			platform.smoothCamera(smoothCameraBefore);
		}
	}

	/**
	 * The mouse wheel while playing: while zooming it zooms further in or
	 * out (and the hotbar doesn't scroll). True when used.
	 */
	public boolean scroll(double amount) {
		if (!zooming || amount == 0) {
			return false;
		}
		float next = amount > 0 ? factor * SCROLL_STEP : factor / SCROLL_STEP;
		factor = Math.max(MIN_FACTOR, Math.min(MAX_FACTOR, next));
		return true;
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
