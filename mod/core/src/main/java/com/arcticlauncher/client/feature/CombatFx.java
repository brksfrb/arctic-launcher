package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Feedback for fights: a marker at the crosshair when you hit, sounds for hits and kills, and a
 * red pulse at the screen edges when your health is low. Only shows and plays what the game
 * already told the client; it changes nothing about the fight.
 */
public final class CombatFx {
	/** Names of the sound choices (0 is off); the platform plays them by number. */
	public static final String[] SOUNDS = {"Off", "Ding", "Orb", "Level up"};
	/** The marker colors to pick from. */
	public static final int[] MARKER_COLORS = {0xFFFFFFFF, 0xFFFFD84D, 0xFF5CE1E6, 0xFF86EFAC, 0xFFFF6767};

	private static final long HIT_MS = 260;
	private static final long KILL_MS = 520;
	/** A target that dies this long after your last hit counts as your kill. */
	private static final long KILL_WINDOW_MS = 3000;
	private static final int KILL_COLOR = 0xFFFF3B3B;
	private static final long BEEP_EVERY_MS = 1400;
	private static final int EDGE_STEPS = 10;
	private static final int EDGE_STEP_PX = 6;

	private long hitAt;
	private long killAt;
	private boolean killCounted = true;
	private long beepAt;

	/** You hit something (the game's attack, whatever the server then decides). */
	public void hit(ClientConfig c, Platform platform) {
		hitAt = System.currentTimeMillis();
		killCounted = false;
		if (c.hitSound > 0) {
			platform.playSound(c.hitSound, 1f);
		}
	}

	/** Each game tick in a world: a target of yours dying, and the low-health beep. */
	public void tick(ClientConfig c, Platform platform) {
		long now = System.currentTimeMillis();
		if (!killCounted && now - hitAt <= KILL_WINDOW_MS && platform.lastTargetDead()) {
			killCounted = true;
			killAt = now;
			if (c.killSound > 0) {
				platform.playSound(c.killSound, 0.8f);
			}
		}
		if (c.lowHealthWarning && c.lowHealthBeep && low(c, platform) && now - beepAt >= BEEP_EVERY_MS) {
			beepAt = now;
			platform.playSound(2, 0.5f);
		}
	}

	private static boolean low(ClientConfig c, Platform platform) {
		float[] h = platform.health();
		return h != null && h[0] > 0 && h[0] <= c.lowHealthHearts * 2f;
	}

	/** The marker and the edge pulse (the in-world HUD, before the widgets). */
	public void render(Gfx g, ClientConfig c, Platform platform) {
		long now = System.currentTimeMillis();
		if (c.hitMarker) {
			marker(g, c, now);
		}
		if (c.lowHealthWarning && low(c, platform)) {
			edges(g, now);
		}
	}

	private void marker(Gfx g, ClientConfig c, long now) {
		boolean kill = now - killAt < KILL_MS;
		long age = kill ? now - killAt : now - hitAt;
		long length = kill ? KILL_MS : HIT_MS;
		if (age < 0 || age >= length) {
			return;
		}
		int color = kill ? KILL_COLOR : c.hitMarkerColor;
		int alpha = (int) (255 * (1 - age / (double) length));
		int rgb = (alpha << 24) | (color & 0xFFFFFF);
		int cx = g.width() / 2;
		int cy = g.height() / 2;
		int gap = 3 + (int) (age * 3 / length);
		int len = kill ? 5 : 4;
		int shadow = ((alpha * 2 / 3) << 24) | 0x000000;
		for (int i = 0; i < len; i++) {
			int d = gap + i * 2;
			dot(g, cx - d, cy - d, shadow, rgb);
			dot(g, cx + d, cy - d, shadow, rgb);
			dot(g, cx - d, cy + d, shadow, rgb);
			dot(g, cx + d, cy + d, shadow, rgb);
		}
	}

	/** A 2×2 point with a dark 1-pixel shadow, so it shows on bright and dark ground. */
	private static void dot(Gfx g, int x, int y, int shadow, int color) {
		g.fill(x, y, x + 3, y + 3, shadow);
		g.fill(x, y, x + 2, y + 2, color);
	}

	/** A red frame around the screen that pulses; stronger the closer to the edge. */
	private static void edges(Gfx g, long now) {
		double pulse = 0.6 + 0.4 * Math.sin(now / 260.0);
		int w = g.width();
		int h = g.height();
		for (int i = 0; i < EDGE_STEPS; i++) {
			double strength = (1 - i / (double) EDGE_STEPS) * pulse * 0.85;
			int rgb = ((int) (255 * strength) << 24) | 0xD01010;
			int a = i * EDGE_STEP_PX;
			int b = a + EDGE_STEP_PX;
			g.fill(a, a, w - a, b, rgb);
			g.fill(a, h - b, w - a, h - a, rgb);
			g.fill(a, b, b, h - b, rgb);
			g.fill(w - b, b, w - a, h - b, rgb);
		}
	}
}
