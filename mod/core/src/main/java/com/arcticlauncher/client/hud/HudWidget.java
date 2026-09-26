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
	/** Draw the backdrop panel (set per widget by the player). */
	protected boolean background = true;

	protected HudWidget(String id, String name, String description, boolean onByDefault, Column column) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.onByDefault = onByDefault;
		this.column = column;
	}

	/** The widget's backdrop, unless the player turned backgrounds off. */
	protected void panel(Gfx g, Style s, int x0, int y0, int x1, int y1) {
		if (background) {
			com.arcticlauncher.client.gfx.Draw.round(g, x0, y0, x1, y1, 2, s.hud);
		}
	}

	/** Text needs a shadow to read without a backdrop. */
	protected boolean shadow() {
		return !background;
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
