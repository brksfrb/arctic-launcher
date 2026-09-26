package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/**
 * The HUD widgets, where they sit, and drawing them. Widgets the player
 * hasn't moved stack in their column; moved ones keep their spot.
 */
public final class Hud {
	public static final int MARGIN = 4;
	public static final int GAP = 2;
	/** Stacks stop above the hotbar and chat, then continue in a new column. */
	private static final int BOTTOM_RESERVE = 64;
	public static final float MIN_SCALE = 0.5f;
	public static final float MAX_SCALE = 2.5f;

	private final ClientConfig config;
	private final Cps cps = new Cps();
	private final List<HudWidget> widgets;

	public Hud(ClientConfig config) {
		this.config = config;
		List<HudWidget> all = new ArrayList<HudWidget>();
		for (HudWidget w : Widgets.all(cps)) {
			if (!w.needsGame() || ArcticClient.platform().hasFeatures()) {
				all.add(w);
			}
		}
		widgets = Collections.unmodifiableList(all);
	}

	public List<HudWidget> widgets() {
		return widgets;
	}

	public Cps cps() {
		return cps;
	}

	/** The widget's placement, created from its defaults on first use. */
	public HudSlot slot(HudWidget w) {
		HudSlot slot = config.hud.get(w.id);
		if (slot == null) {
			slot = new HudSlot(w.onByDefault);
			config.hud.put(w.id, slot);
		}
		return slot;
	}

	/** Back to the default set of widgets, neatly stacked. */
	public void reset() {
		config.hud.clear();
	}

	/** Every shown widget's rectangle {x, y, w, h}, in drawing order. */
	public Map<HudWidget, int[]> layout(Gfx g) {
		Map<HudWidget, int[]> rects = new IdentityHashMap<HudWidget, int[]>();
		Stack left = new Stack();
		Stack right = new Stack();
		int bottom = Math.max(MARGIN + 20, g.height() - BOTTOM_RESERVE);
		for (HudWidget w : widgets) {
			HudSlot slot = slot(w);
			if (!slot.enabled) {
				continue;
			}
			int ww = Math.round(w.width(g) * slot.scale);
			int hh = Math.round(w.height() * slot.scale);
			int x;
			int y;
			if (slot.placed) {
				x = anchored(slot.ax, slot.dx, g.width(), ww);
				y = anchored(slot.ay, slot.dy, g.height(), hh);
			} else {
				Stack stack = w.column == HudWidget.Column.LEFT ? left : right;
				stack.fit(hh, bottom);
				x = w.column == HudWidget.Column.LEFT ? stack.x : g.width() - stack.x - ww;
				y = stack.y;
				stack.take(ww, hh);
			}
			x = Math.max(0, Math.min(g.width() - ww, x));
			y = Math.max(0, Math.min(g.height() - hh, y));
			rects.put(w, new int[] {x, y, ww, hh});
		}
		return rects;
	}

	/** Where the next unmoved widget goes in a column (x from its screen edge). */
	private static final class Stack {
		int x = MARGIN;
		int y = MARGIN;
		int widest;

		/** Start a new column when this widget would reach the bottom. */
		void fit(int h, int bottom) {
			if (y > MARGIN && y + h > bottom) {
				x += widest + GAP;
				y = MARGIN;
				widest = 0;
			}
		}

		void take(int w, int h) {
			y += h + GAP;
			widest = Math.max(widest, w);
		}
	}

	private static int anchored(int anchor, int offset, int screen, int size) {
		if (anchor == HudSlot.START) {
			return offset;
		}
		if (anchor == HudSlot.END) {
			return screen - size - offset;
		}
		return (screen - size) / 2 + offset;
	}

	/**
	 * Pin a widget with its top-left at (x, y), anchored to the nearest
	 * screen edge (or the middle) so it stays there on other screen sizes.
	 */
	public void place(Gfx g, HudWidget w, int[] rect, int x, int y) {
		HudSlot slot = slot(w);
		slot.placed = true;
		slot.ax = third(x + rect[2] / 2, g.width());
		slot.ay = third(y + rect[3] / 2, g.height());
		slot.dx = offset(slot.ax, x, g.width(), rect[2]);
		slot.dy = offset(slot.ay, y, g.height(), rect[3]);
	}

	private static int third(int center, int screen) {
		if (center < screen / 3) {
			return HudSlot.START;
		}
		return center > screen * 2 / 3 ? HudSlot.END : HudSlot.CENTER;
	}

	private static int offset(int anchor, int pos, int screen, int size) {
		if (anchor == HudSlot.START) {
			return pos;
		}
		if (anchor == HudSlot.END) {
			return screen - size - pos;
		}
		return pos - (screen - size) / 2;
	}

	public void render(Gfx g, Style s, boolean preview) {
		for (Map.Entry<HudWidget, int[]> e : sorted(layout(g))) {
			draw(g, s, e.getKey(), e.getValue(), preview);
		}
	}

	/** Layout entries in widget order (the map itself has none). */
	public List<Map.Entry<HudWidget, int[]>> sorted(Map<HudWidget, int[]> rects) {
		List<Map.Entry<HudWidget, int[]>> out = new ArrayList<Map.Entry<HudWidget, int[]>>();
		for (HudWidget w : widgets) {
			int[] r = rects.get(w);
			if (r != null) {
				out.add(new java.util.AbstractMap.SimpleEntry<HudWidget, int[]>(w, r));
			}
		}
		return out;
	}

	public void draw(Gfx g, Style s, HudWidget w, int[] rect, boolean preview) {
		g.push();
		g.translate(rect[0], rect[1]);
		g.scale(slot(w).scale);
		w.render(g, s, preview);
		g.pop();
	}
}
