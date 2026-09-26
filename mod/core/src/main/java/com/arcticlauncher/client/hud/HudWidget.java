package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** One HUD element (FPS, keystrokes, …), drawn at its own top-left. */
public abstract class HudWidget {
	public final String id;
	public final String name;
	public final String description;
	final HudSlot defaults;

	protected HudWidget(String id, String name, String description, HudSlot defaults) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.defaults = defaults;
	}

	public abstract int width(Gfx g);

	public abstract int height();

	/**
	 * Draw at (0, 0). {@code preview} is true in the layout editor, where
	 * widgets show sample values when there's nothing live to show.
	 */
	public abstract void render(Gfx g, Style s, boolean preview);
}
