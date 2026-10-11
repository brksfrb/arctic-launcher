package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * A part of the game's own HUD (the scoreboard, the boss bar) that the HUD editor can move,
 * resize and turn off like a widget. The game still draws it: the version adapter reports where
 * it would go and draws it where this says ({@link Hud#vanillaPlacement}). Until it's moved it
 * stays where the game puts it.
 */
final class VanillaPart extends HudWidget {
	/** Not reported for this long: the game isn't showing it (no scoreboard, no boss). */
	private static final long SHOWN_MS = 1000;

	private final int sampleW;
	private final int sampleH;
	private volatile int[] natural;
	private volatile long seenAt;

	VanillaPart(String id, String name, String description, int sampleW, int sampleH) {
		super(id, name, description, true, Column.RIGHT);
		this.sampleW = sampleW;
		this.sampleH = sampleH;
	}

	@Override
	public boolean needsGame() {
		return true;
	}

	/** Where the game would draw it this frame. */
	void saw(int x, int y, int w, int h) {
		natural = new int[] {x, y, w, h};
		seenAt = System.currentTimeMillis();
	}

	private int[] showing() {
		int[] n = natural;
		return n != null && System.currentTimeMillis() - seenAt < SHOWN_MS ? n : null;
	}

	@Override
	public int width(Gfx g) {
		int[] n = showing();
		return n != null ? n[2] : sampleW;
	}

	@Override
	public int height() {
		int[] n = showing();
		return n != null ? n[3] : sampleH;
	}

	@Override
	public int[] home(Gfx g) {
		int[] n = showing();
		if (n != null) {
			return n.clone();
		}
		if (!ArcticClient.hud().previewing()) {
			// Nothing on screen: keep no room for it.
			return new int[] {0, 0, 0, 0};
		}
		if (VanillaParts.BOSSBAR.equals(id)) {
			return new int[] {(g.width() - sampleW) / 2, 3, sampleW, sampleH};
		}
		return new int[] {g.width() - sampleW - 1, (g.height() - sampleH) / 2, sampleW, sampleH};
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		// The game draws the real one; the editor shows a stand-in when there's none to see.
		if (!preview || showing() != null) {
			return;
		}
		if (VanillaParts.BOSSBAR.equals(id)) {
			Draw.centered(g, "Boss bar", sampleW / 2, 0, 0xFFFFFFFF, true);
			g.fill(0, 9, sampleW, 14, 0xFF4A1020);
			g.fill(0, 9, sampleW * 2 / 3, 14, 0xFFE0365B);
			return;
		}
		g.fill(0, 0, sampleW, 10, 0x66000000);
		g.fill(0, 10, sampleW, sampleH, 0x4D000000);
		Draw.centered(g, "Scoreboard", sampleW / 2, 1, 0xFFFFFFFF, false);
		String[] rows = {"Kills", "Coins", "Team"};
		for (int i = 0; i < rows.length; i++) {
			int y = 11 + i * 9;
			g.text(rows[i], 2, y, 0xFFAAAAAA, false);
			String score = String.valueOf(12 - i * 4);
			g.text(score, sampleW - 2 - g.textWidth(score), y, 0xFFFF5555, false);
		}
	}
}
