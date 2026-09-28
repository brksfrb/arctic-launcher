package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * WASD, the mouse buttons with clicks per second, and the space bar. A key
 * turns white the moment it's pressed and fades back when let go.
 */
final class Keystrokes extends HudWidget {
	private static final int KEY = 22;
	private static final int GAP = 2;
	private static final int MOUSE_H = 22;
	private static final int SPACE_H = 11;
	private static final int WIDTH = KEY * 3 + GAP * 2;
	/** How fast a released key fades back (per second). */
	private static final float FADE = 6f;
	private static final int PRESSED = 0xFFFFFFFF;
	private static final int PRESSED_TEXT = 0xFF000000;
	private static final int SMALL_PX = 6;

	private final Cps cps;
	/** How lit each key is: W A S D, LMB, RMB, space. */
	private final float[] lit = new float[7];
	private long lastFrame;

	Keystrokes(Cps cps) {
		super("keystrokes", "Keystrokes", "Movement keys and mouse buttons", true, Column.RIGHT);
		this.cps = cps;
	}

	@Override
	public int width(Gfx g) {
		return WIDTH;
	}

	@Override
	public int height() {
		return KEY * 2 + MOUSE_H + SPACE_H + GAP * 3;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		boolean[] down = {
				down(GameKey.FORWARD), down(GameKey.LEFT), down(GameKey.BACK), down(GameKey.RIGHT),
				down(GameKey.ATTACK), down(GameKey.USE), down(GameKey.JUMP),
		};
		for (int i = 0; i < lit.length; i++) {
			lit[i] = down[i] ? 1f : Math.max(0f, lit[i] - dt * FADE);
		}
		key(g, s, KEY + GAP, 0, KEY, KEY, "W", null, lit[0]);
		int row = KEY + GAP;
		key(g, s, 0, row, KEY, KEY, "A", null, lit[1]);
		key(g, s, KEY + GAP, row, KEY, KEY, "S", null, lit[2]);
		key(g, s, (KEY + GAP) * 2, row, KEY, KEY, "D", null, lit[3]);
		row += KEY + GAP;
		int half = (WIDTH - GAP) / 2;
		key(g, s, 0, row, half, MOUSE_H, "LMB", cps.get(Keys.MOUSE_LEFT) + " CPS", lit[4]);
		key(g, s, half + GAP, row, WIDTH - half - GAP, MOUSE_H, "RMB", cps.get(Keys.MOUSE_RIGHT) + " CPS", lit[5]);
		row += MOUSE_H + GAP;
		key(g, s, 0, row, WIDTH, SPACE_H, null, null, lit[6]);
		int bar = Draw.mix(text(), PRESSED_TEXT, lit[6]);
		g.fill(WIDTH / 2 - 10, row + SPACE_H / 2, WIDTH / 2 + 10, row + SPACE_H / 2 + 1, bar);
	}

	private static boolean down(GameKey key) {
		return ArcticClient.platform().inWorld() && ArcticClient.platform().keyDown(key);
	}

	/** One key: its box (white while lit), its label and an optional small line under it. */
	private void key(Gfx g, Style s, int x, int y, int w, int h, String label, String small, float lit) {
		panel(g, s, x, y, x + w, y + h);
		if (lit > 0f) {
			g.fill(x, y, x + w, y + h, Draw.alpha(PRESSED, lit * 0.9f));
		}
		int color = Draw.mix(text(), PRESSED_TEXT, lit);
		boolean shadow = shadow() && lit < 0.5f;
		if (label == null) {
			return;
		}
		if (small == null) {
			Draw.centered(g, label, x + w / 2, y + (h - 8) / 2, color, shadow);
			return;
		}
		Draw.centered(g, label, x + w / 2, y + 3, color, shadow);
		// The CPS line, drawn a little smaller.
		g.push();
		g.translate(x + w / 2f, y + h - 3 - SMALL_PX);
		g.scale(0.75f);
		Draw.centered(g, small, 0, 0, Draw.alpha(color, 0.85f), shadow);
		g.pop();
	}
}
