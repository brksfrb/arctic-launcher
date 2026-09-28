package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** One tab of the Arctic menu. */
interface MenuTab {
	/** The sidebar name. */
	String title();

	/** The line under the title. */
	String hint();

	/** Only makes sense in a world (hidden on the title screen's menu). */
	default boolean needsWorld() {
		return false;
	}

	/** Lay out the tab's widgets in the content area. */
	void build(Host host, int x, int top, int w, int bottom);

	/** Anything drawn by hand, under the widgets. */
	default void draw(Gfx g, Style s, int mx, int my) {}

	/** Anything drawn by hand, over the widgets (previews). */
	default void drawAbove(Gfx g, Style s, int mx, int my) {}

	/** What the tab shows; the menu rebuilds when this changes (null: never). */
	default String state() {
		return null;
	}

	/** Replaces the hint while something's happening (null: the hint). */
	default String status() {
		return null;
	}

	/** Rebuild on a state change even while typing (the tab keeps its text box across rebuilds). */
	default boolean rebuildWhileTyping() {
		return false;
	}

	/** Called 20 times a second while the tab is open. */
	default void tick(Host host) {}
}
