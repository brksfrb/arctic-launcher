package com.arcticlauncher.client.feature;

import java.util.Map;
import java.util.WeakHashMap;

/**
 * The laid-out lines of each sign, kept between frames (1.8.9 to 1.12.2 lay out every line of every
 * sign in view again each frame: wrapping it to the board's width, measuring every character).
 *
 * <p>A line's layout depends only on the line itself and on the font: it is kept for as long as the
 * sign holds that very line object (an update from the server, or typing on it, puts new ones in) and
 * the font measures the same ({@code fontKey}: the font object and the width of a probe string, which
 * changes when the font or the resource packs do). Signs that are unloaded drop out on their own.
 * Render thread only.
 */
public final class SignLineCache {
	/** Lines one sign keeps (a sign has four). */
	static final int LINES = 4;

	private final Map<Object, Entry> signs = new WeakHashMap<Object, Entry>();
	private Object font;
	private float fontKey = Float.NaN;

	private static final class Entry {
		final Object[] lines = new Object[LINES];
		final Object[] laidOut = new Object[LINES];
		int next;
	}

	/**
	 * The font about to be used, measured: when it differs from the last one every kept layout is
	 * dropped. Call once a frame, before the signs are drawn.
	 */
	public void font(Object font, float probeWidth) {
		if (font != this.font || Float.compare(probeWidth, fontKey) != 0) {
			signs.clear();
			this.font = font;
			fontKey = probeWidth;
		}
	}

	/** The kept layout of {@code line} on {@code sign}, or null when there is none. */
	public Object get(Object sign, Object line) {
		if (sign == null || line == null) {
			return null;
		}
		Entry e = signs.get(sign);
		if (e == null) {
			return null;
		}
		for (int i = 0; i < LINES; i++) {
			if (e.lines[i] == line) {
				return e.laidOut[i];
			}
		}
		return null;
	}

	/** Keep {@code laidOut} as the layout of {@code line} on {@code sign}. */
	public void put(Object sign, Object line, Object laidOut) {
		if (sign == null || line == null || laidOut == null) {
			return;
		}
		Entry e = signs.get(sign);
		if (e == null) {
			e = new Entry();
			signs.put(sign, e);
		}
		for (int i = 0; i < LINES; i++) {
			if (e.lines[i] == line) {
				e.laidOut[i] = laidOut;
				return;
			}
		}
		// A line not kept yet replaces the oldest one (the sign's four lines take the four places).
		e.lines[e.next] = line;
		e.laidOut[e.next] = laidOut;
		e.next = (e.next + 1) % LINES;
	}

	/** How many signs have kept lines. */
	public int size() {
		return signs.size();
	}
}
