package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;
import java.util.function.IntSupplier;

/** A 3x3 wall of joined clear glass over sky and grass, drawn with the tiles the game gets at the current line level. */
final class GlassPreview extends Widget {
	static final int BLOCKS = 3, SCALE = 2, MARGIN = 8, SIZE = BLOCKS * 16 * SCALE + MARGIN * 2;
	private final IntSupplier level;
	private int shown = -1;
	private int[] pixels;

	GlassPreview(IntSupplier level) {
		this.level = level;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int now = level.getAsInt();
		if (now != shown) {
			pixels = GlassTiles.preview(BLOCKS, now);
			shown = now;
		}
		int horizon = y + h * 3 / 5;
		g.fill(x, y, x + w, horizon, 0xFF8DB3EA);
		g.fill(x, horizon, x + w, y + h, 0xFF5C8B34);
		g.fill(x + w / 2 - 5, y + h / 4, x + w / 2 + 5, horizon + 6, 0xFF6B4F2A);
		g.fill(x + w / 2 - 16, y + 6, x + w / 2 + 16, y + h / 4 + 6, 0xFF2F6B24);
		int size = BLOCKS * 16, left = x + (w - size * SCALE) / 2, top = y + (h - size * SCALE) / 2;
		for (int py = 0; py < size; py++) {
			for (int px = 0; px < size; ) {
				int color = pixels[py * size + px], run = px + 1;
				while (run < size && pixels[py * size + run] == color) run++;
				if (color >>> 24 != 0) g.fill(left + px * SCALE, top + py * SCALE, left + run * SCALE, top + (py + 1) * SCALE, color);
				px = run;
			}
		}
	}
}
