package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** WASD, mouse buttons (with CPS) and the space bar, lit while held. */
final class Keystrokes extends HudWidget {
	private static final int KEY = 18;
	private static final int GAP = 2;
	private static final int MOUSE_H = 14;
	private static final int SPACE_H = 8;
	private static final int WIDTH = KEY * 3 + GAP * 2;

	private final Cps cps;

	Keystrokes(Cps cps) {
		super("keystrokes", "Keystrokes", "Movement keys and mouse buttons", new HudSlot(true, 1f, 0.35f));
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
		key(g, s, KEY + GAP, 0, KEY, KEY, "W", down(GameKey.FORWARD));
		int row = KEY + GAP;
		key(g, s, 0, row, KEY, KEY, "A", down(GameKey.LEFT));
		key(g, s, KEY + GAP, row, KEY, KEY, "S", down(GameKey.BACK));
		key(g, s, (KEY + GAP) * 2, row, KEY, KEY, "D", down(GameKey.RIGHT));
		row += KEY + GAP;
		int half = (WIDTH - GAP) / 2;
		key(g, s, 0, row, half, MOUSE_H, mouse("LMB", Keys.MOUSE_LEFT), down(GameKey.ATTACK));
		key(g, s, half + GAP, row, WIDTH - half - GAP, MOUSE_H, mouse("RMB", Keys.MOUSE_RIGHT), down(GameKey.USE));
		row += MOUSE_H + GAP;
		boolean jump = down(GameKey.JUMP);
		key(g, s, 0, row, WIDTH, SPACE_H, null, jump);
		int bar = jump ? s.onAccent : s.muted;
		g.fill(WIDTH / 2 - 8, row + SPACE_H / 2, WIDTH / 2 + 8, row + SPACE_H / 2 + 1, bar);
	}

	private String mouse(String name, int button) {
		int n = cps.get(button);
		// While clicking, the button shows its clicks per second.
		return n > 0 ? String.valueOf(n) : name;
	}

	private static boolean down(GameKey key) {
		return ArcticClient.platform().inWorld() && ArcticClient.platform().keyDown(key);
	}

	private static void key(Gfx g, Style s, int x, int y, int w, int h, String label, boolean pressed) {
		Draw.round(g, x, y, x + w, y + h, 2, pressed ? Draw.alpha(s.accent, 0.85f) : s.hud);
		if (label != null) {
			int color = pressed ? s.onAccent : s.text;
			Draw.centered(g, label, x + w / 2, y + (h - 8) / 2, color, false);
		}
	}
}
