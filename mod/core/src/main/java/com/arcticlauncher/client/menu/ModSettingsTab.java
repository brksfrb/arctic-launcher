package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.style.Style;

/** A mod's settings; HUD widgets get a live preview beside them. */
final class ModSettingsTab implements MenuTab {
	private static final int PREVIEW_W = 150;
	private static final int GAP = 10;

	private final Mod mod;
	private int[] preview;

	ModSettingsTab(Mod mod) {
		this.mod = mod;
	}

	@Override
	public String title() {
		return mod.name;
	}

	@Override
	public String hint() {
		return mod.description;
	}

	@Override
	public void build(Host host, int x, int top, int w, int bottom) {
		int formW = w;
		preview = null;
		if (mod.widget != null) {
			formW = w - PREVIEW_W - GAP;
			preview = new int[] {x + formW + GAP, top, PREVIEW_W, Math.min(110, bottom - top)};
		}
		Form f = new Form(host, x, top, formW, bottom);
		if (mod.settings != null) {
			mod.settings.build(host, f);
		}
		f.done();
	}

	/** The widget as it looks now, centered on a sky-and-grass backdrop. */
	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		if (preview == null) {
			return;
		}
		int x0 = preview[0];
		int y0 = preview[1];
		int x1 = x0 + preview[2];
		int y1 = y0 + preview[3];
		g.gradient(x0, y0, x1, y1, 0xFF6B9BD8, 0xFFA9C8F0);
		g.fill(x0, y0 + preview[3] * 2 / 3, x1, y1, 0xFF5E9E3B);
		Draw.outline(g, x0, y0, x1, y1, 1, s.border);
		Hud hud = ArcticClient.hud();
		float scale = hud.slot(mod.widget).scale;
		int ww = Math.round(mod.widget.width(g) * scale);
		int hh = Math.round(mod.widget.height() * scale);
		int[] rect = {(x0 + x1 - ww) / 2, (y0 + y1 - hh) / 2, ww, hh};
		g.scissor(x0, y0, x1, y1);
		hud.draw(g, s, mod.widget, rect, true);
		g.endScissor();
		Draw.centered(g, "Preview", (x0 + x1) / 2, y1 + 4, s.muted, false);
	}
}
