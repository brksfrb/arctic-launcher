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
	/** Each widget's slot, looked up once (the config map is keyed by id strings). */
	private final Map<HudWidget, HudSlot> slots = new IdentityHashMap<HudWidget, HudSlot>();
	private Map<String, HudSlot> slotsFrom;
	private int slotsEpoch;
	/** Counts HUD draws: text widgets work their value and width out once per draw. */
	private static long frame;
	/** Per widget: what it showed, its size, and its last drawing (see {@link HudWidget#content}). */
	private final Map<HudWidget, Kept> kept = new IdentityHashMap<HudWidget, Kept>();
	/** The last layout, and what it was worked out from (screen size; each widget's slot and size). */
	private Map<HudWidget, int[]> lastLayout;
	private long[] lastLayoutFrom = new long[0];
	private List<Map.Entry<HudWidget, int[]>> lastSorted;
	private Map<HudWidget, int[]> lastSortedFrom;

	private static final class Kept {
		/** This frame's content (null: changes every frame). */
		long frame = -1;
		boolean preview;
		Object content;
		/** The size worked out for {@link #sizedContent}. */
		Object sizedContent;
		long sizedLook;
		boolean sizedFancy;
		int width;
		int height;
		/** The last drawing, and what it showed where. */
		Object drawing;
		Object drawnContent;
		long drawnLook;
		boolean drawnPreview;
		boolean drawnFancy;
		Style drawnStyle;
		int[] drawnRect;
	}

	/** The widget's state for this frame: its content asked for once. */
	private Kept kept(HudWidget w, HudSlot slot, boolean preview) {
		Kept k = kept.get(w);
		if (k == null) {
			k = new Kept();
			kept.put(w, k);
		}
		if (k.frame != frame || k.preview != preview) {
			w.use(slot);
			k.content = w.content(preview);
			k.frame = frame;
			k.preview = preview;
		}
		return k;
	}

	/** Width and height (unscaled): worked out again only when what it shows, or its look, changed. */
	private Kept sized(Gfx g, HudWidget w, HudSlot slot) {
		Kept k = kept(w, slot, previewing);
		long look = slot.look();
		boolean fancy = com.arcticlauncher.client.gfx.Draw.fancy;
		if (k.content == null || !k.content.equals(k.sizedContent) || k.sizedLook != look || k.sizedFancy != fancy) {
			w.use(slot);
			k.width = w.width(g);
			k.height = w.height();
			k.sizedContent = k.content;
			k.sizedLook = look;
			k.sizedFancy = fancy;
		}
		return k;
	}

	/** Whether this draw is the editor's preview (sample values). */
	private boolean previewing;

	static long frame() {
		return frame;
	}

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
		if (slotsFrom != config.hud || slotsEpoch != config.hudEpoch) {
			slots.clear();
			slotsFrom = config.hud;
			slotsEpoch = config.hudEpoch;
		}
		HudSlot slot = slots.get(w);
		if (slot != null) {
			return slot;
		}
		slot = config.hud.get(w.id);
		if (slot == null) {
			slot = new HudSlot(w.onByDefault);
			config.hud.put(w.id, slot);
		}
		if (!slot.styled) {
			// A widget seen for the first time (new in this version, or never shown) looks like the rest:
			// the HUD style last picked, not plain boxes. One the player restyled by hand is left alone.
			com.arcticlauncher.client.style.Style style = com.arcticlauncher.client.ArcticClient.style();
			if (style != null) {
				if (config.hudStyle != null && slot.looksPlain()) {
					HudStyles.apply(config.hudStyle, slot, style);
				}
				slot.styled = true;
			}
		}
		slots.put(w, slot);
		return slot;
	}

	/** Back to the default set of widgets, neatly stacked. */
	public void reset() {
		config.hud.clear();
		config.hudChanged();
	}

	/**
	 * Every shown widget's rectangle {x, y, w, h}. Moved widgets go where
	 * they were put; the rest stack in their column, stepping around them.
	 */
	public Map<HudWidget, int[]> layout(Gfx g) {
		frame++;
		// Worked out again only when the screen, a widget's slot or a widget's size changed.
		long[] from = new long[2 + widgets.size() * 3];
		from[0] = g.width();
		from[1] = g.height();
		int f = 2;
		for (HudWidget w : widgets) {
			HudSlot slot = slot(w);
			from[f++] = slot.look();
			if (slot.enabled) {
				Kept k = sized(g, w, slot);
				from[f++] = k.width;
				from[f++] = k.height;
			} else {
				f += 2;
			}
		}
		if (lastLayout != null && java.util.Arrays.equals(from, lastLayoutFrom)) {
			return lastLayout;
		}
		lastLayoutFrom = from;
		lastLayout = workOutLayout(g);
		return lastLayout;
	}

	private Map<HudWidget, int[]> workOutLayout(Gfx g) {
		Map<HudWidget, int[]> rects = new IdentityHashMap<HudWidget, int[]>();
		List<int[]> taken = new ArrayList<int[]>();
		for (HudWidget w : widgets) {
			HudSlot slot = slot(w);
			if (slot.enabled && slot.placed) {
				Kept k = sized(g, w, slot);
				int ww = Math.round(k.width * slot.scale);
				int hh = Math.round(k.height * slot.scale);
				int x = clamp(anchored(slot.ax, slot.dx, g.width(), ww), g.width() - ww);
				int y = clamp(anchored(slot.ay, slot.dy, g.height(), hh), g.height() - hh);
				int[] r = {x, y, ww, hh};
				rects.put(w, r);
				taken.add(r);
			}
		}
		Stack left = new Stack();
		Stack right = new Stack();
		int bottom = Math.max(MARGIN + 20, g.height() - BOTTOM_RESERVE);
		for (HudWidget w : widgets) {
			HudSlot slot = slot(w);
			if (!slot.enabled || slot.placed) {
				continue;
			}
			Kept k = sized(g, w, slot);
			int ww = Math.round(k.width * slot.scale);
			int hh = Math.round(k.height * slot.scale);
			boolean onLeft = w.column == HudWidget.Column.LEFT;
			Stack stack = onLeft ? left : right;
			int[] r = new int[4];
			for (int tries = 0; tries < 64; tries++) {
				stack.fit(hh, bottom);
				int x = onLeft ? stack.x : g.width() - stack.x - ww;
				r = new int[] {clamp(x, g.width() - ww), clamp(stack.y, g.height() - hh), ww, hh};
				int[] hit = overlap(r, taken);
				if (hit == null) {
					break;
				}
				// Step below whatever is in the way and try again.
				stack.y = hit[1] + hit[3] + GAP;
			}
			stack.take(ww, hh);
			rects.put(w, r);
			taken.add(r);
		}
		return rects;
	}

	private static int clamp(int v, int max) {
		return Math.max(0, Math.min(max, v));
	}

	/** The first rectangle in {@code others} that overlaps {@code r}, or null. */
	private static int[] overlap(int[] r, List<int[]> others) {
		for (int[] o : others) {
			if (r[0] < o[0] + o[2] && o[0] < r[0] + r[2] && r[1] < o[1] + o[3] && o[1] < r[1] + r[3]) {
				return o;
			}
		}
		return null;
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
		previewing = preview;
		Map<HudWidget, int[]> rects = layout(g);
		if (lastSorted == null || lastSortedFrom != rects) {
			lastSorted = sorted(rects);
			lastSortedFrom = rects;
		}
		for (Map.Entry<HudWidget, int[]> e : lastSorted) {
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
		g.newLayer();
		HudSlot slot = slot(w);
		Kept k = kept(w, slot, preview);
		long look = slot.look();
		boolean fancy = com.arcticlauncher.client.gfx.Draw.fancy;
		// Showing the same thing, the same way, in the same place: last frame's drawing again.
		if (k.content != null && k.drawing != null && k.content.equals(k.drawnContent) && k.drawnLook == look && k.drawnPreview == preview
				&& k.drawnFancy == fancy && k.drawnStyle == s && java.util.Arrays.equals(rect, k.drawnRect) && g.replay(k.drawing)) {
			return;
		}
		boolean recording = k.content != null && g.startRecording();
		try {
			g.push();
			g.translate(rect[0], rect[1]);
			g.scale(slot.scale);
			w.use(slot);
			w.render(g, s, preview);
			g.pop();
		} finally {
			k.drawing = recording ? g.stopRecording() : null;
		}
		k.drawnContent = k.content;
		k.drawnLook = look;
		k.drawnPreview = preview;
		k.drawnFancy = fancy;
		k.drawnStyle = s;
		k.drawnRect = rect.clone();
	}
}
