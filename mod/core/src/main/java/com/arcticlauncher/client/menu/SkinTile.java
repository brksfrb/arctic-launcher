package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;

/** A skin from your launcher library (or "Minecraft skin"), showing its face. */
final class SkinTile extends Widget {
	private static final int SKIN_SIZE = 64;
	private static final int FACE = 8;

	private final String id;
	private final String name;
	private final String texture;
	private final boolean worn;

	SkinTile(String id, String name, String texture, boolean worn) {
		this.id = id;
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
		int size = Math.min(w - 12, h - 22);
		int fx = x + (w - size) / 2;
		int fy = y + 5;
		if (texture != null && ArcticClient.looks().isReady(texture)) {
			String key = "look:" + texture;
			// The face (8,8) and the hat layer over it (40,8).
			g.texture(key, fx, fy, size, size, FACE, FACE, FACE, FACE, SKIN_SIZE, SKIN_SIZE);
			g.texture(key, fx, fy, size, size, FACE * 5, FACE, FACE, FACE, SKIN_SIZE, SKIN_SIZE);
		} else if (texture == null) {
			Draw.centered(g, "MC", x + w / 2, fy + size / 2 - 4, s.muted, false);
		}
		Draw.centered(g, Draw.fitCentered(g, name, w - 4, x + w / 2, y + h - 11), x + w / 2, y + h - 11, worn ? s.accent : s.text, false);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT || worn || !enabled) {
			return false;
		}
		ArcticClient.launcherSkins().wear(id);
		return true;
	}
}
