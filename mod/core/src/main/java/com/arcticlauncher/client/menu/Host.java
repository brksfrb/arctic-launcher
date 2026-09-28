package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.Widget;

/** Where a tab's widgets go: the Arctic menu. */
interface Host {
	Widget add(Widget w);

	/** Lay the whole menu out again (keeps each tab's scroll position). */
	void rebuild();

	/** A key button that should catch the next key press. */
	default void listenKeys(KeyButton button) {}

	/** A scrolling box for this tab's content (one per tab; it remembers where it was). */
	default Scroll scroll(int x0, int y0, int x1, int y1) {
		return new Scroll(x0, y0, x1, y1, 0);
	}
}
