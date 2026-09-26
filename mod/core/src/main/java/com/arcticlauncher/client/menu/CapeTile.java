package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/** A cape to wear (or "No cape"), showing the cape's front. */
final class CapeTile extends Widget {
	private final String presetId;
	private final String name;
	private final String texture;
	private final boolean worn;

	CapeTile(String presetId, String name, String texture, boolean worn) {
		this.presetId = presetId;
		this.name = name;
		this.texture = texture;
		this.worn = worn;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		Skin.button(g, s, x, y, w, h, true, worn ? 1f : hover);
		if (worn) {
			Draw.outline(g, x, y, x + w, y + h, 2, s.accent);
		}
		int capeH = h - 18;
		int capeW = capeH * 10 / 16;
		int cx = x + (w - capeW) / 2;
		if (texture != null && ArcticClient.looks().texture(texture)) {
			// Cape front: (1,1) 10x16 of a 64x32 layout; presets are 2x.
			g.texture("look:" + texture, cx, y + 5, capeW, capeH, 2f, 2f, 20, 32, 128, 64);
		} else if (texture == null) {
			Draw.centered(g, "-", x + w / 2, y + 5 + capeH / 2 - 4, s.muted, false);
		}
		Draw.centered(g, Draw.fit(g, name, w - 4), x + w / 2, y + h - 11, worn ? s.accent : s.text, false);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT || worn || !enabled) {
			return false;
		}
		ArcticClient.looks().wearCape(presetId);
		return true;
	}
}
