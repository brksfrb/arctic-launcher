package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** One HUD element (FPS, keystrokes, …), drawn at its own top-left. */
public abstract class HudWidget {
	/** Which side a widget stacks on until the player moves it. */
	public enum Column {
		LEFT,
		RIGHT
	}

	public final String id;
	public final String name;
	public final String description;
	final boolean onByDefault;
	final Column column;

	protected HudWidget(String id, String name, String description, boolean onByDefault, Column column) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.onByDefault = onByDefault;
		this.column = column;
	}

	/** Needs the version's game hooks (hidden where they don't exist). */
	public boolean needsGame() {
		return false;
	}

	public abstract int width(Gfx g);

	public abstract int height();

	/**
	 * Draw at (0, 0). {@code preview} is true in the layout editor, where
	 * widgets show sample values when there's nothing live to show.
	 */
	public abstract void render(Gfx g, Style s, boolean preview);
}
