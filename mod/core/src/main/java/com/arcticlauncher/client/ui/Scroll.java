package com.arcticlauncher.client.ui;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * A scrolling box of widgets: they're laid out as if the box were endless
 * and shown through a window of it; the mouse wheel (or dragging the bar)
 * moves them. Widgets are added to the page as usual, and here too.
 */
public final class Scroll {
	private static final int STEP = 20;
	private static final int BAR_W = 3;
	private static final int MIN_THUMB = 12;

	private final List<Widget> items = new ArrayList<Widget>();
	private final List<Integer> homes = new ArrayList<Integer>();
	private final int[] box;
	private int offset;
	private int contentBottom;
	private final Widget bar = new Widget() {
		private double grab;
		private boolean dragging;
		@Override protected void draw(Gfx g, Style s, int mx, int my, float dt) {}
		@Override public boolean contains(double mx, double my) {
			return maxOffset() > 0 && super.contains(mx, my);
		}
		@Override public boolean click(double mx, double my, int button) {
			if (button != 0 || !contains(mx, my)) return false;
			int top = thumbTop(), thumb = thumbHeight();
			grab = my >= top && my < top + thumb ? my - top : thumb / 2.0;
			dragging = true;
			return drag(mx, my, button);
		}
		@Override public boolean drag(double mx, double my, int button) {
			if (!dragging || button != 0) return false;
			int travel = box[3] - box[1] - thumbHeight();
			if (travel > 0) scrollTo((int) Math.round((my - box[1] - grab) * maxOffset() / travel));
			return true;
		}
		@Override public void release(double mx, double my, int button) {
			if (button == 0) dragging = false;
		}
	};

	/** A window from (x0, y0) to (x1, y1); starts scrolled by {@code offset}. */
	public Scroll(int x0, int y0, int x1, int y1, int offset) {
		box = new int[] {x0, y0, x1, y1};
		this.offset = Math.max(0, offset);
		contentBottom = y0;
		// A wider hit target around the thin visual bar, outside the list's cards.
		bar.bounds(x1, y0, BAR_W + 6, Math.max(0, y1 - y0));
	}

	/** Register with the page so dragging keeps mouse capture outside the track. */
	public Widget bar() { return bar; }

	private int thumbHeight() {
		int height = Math.max(0, box[3] - box[1]);
		return Math.min(height, Math.max(MIN_THUMB, (int) ((long) height * height / Math.max(1, height + maxOffset()))));
	}

	private int thumbTop() {
		return box[1] + (int) ((long) (box[3] - box[1] - thumbHeight()) * offset / Math.max(1, maxOffset()));
	}

	/** Put a widget in the box (its bounds are where it'd be unscrolled). */
	public <T extends Widget> T add(T w) {
		w.clip = box;
		items.add(w);
		homes.add(w.y);
		contentBottom = Math.max(contentBottom, w.y + w.h);
		w.y -= offset;
		return w;
	}

	/** Content ends here (unscrolled), for extra room below the last widget. */
	public void extendTo(int bottom) {
		contentBottom = Math.max(contentBottom, bottom);
	}

	/** Call after adding everything: keeps the offset in range. */
	public void settle() {
		scrollTo(offset);
	}

	public int offset() {
		return offset;
	}

	private int maxOffset() {
		return Math.max(0, contentBottom - box[3] + 4);
	}

	public boolean contains(double mx, double my) {
		return mx >= box[0] && my >= box[1] && mx < box[2] && my < box[3];
	}

	/** The wheel over the box: true when it moved. */
	public boolean wheel(double mx, double my, double amount) {
		if (!contains(mx, my) || maxOffset() == 0) {
			return false;
		}
		int before = offset;
		scrollTo(offset - (int) Math.round(amount * STEP));
		return offset != before;
	}

	public void scrollTo(int value) {
		int next = Math.max(0, Math.min(maxOffset(), value));
		int delta = next - offset;
		offset = next;
		if (delta != 0) {
			for (int i = 0; i < items.size(); i++) {
				items.get(i).y = homes.get(i) - offset;
			}
		}
	}

	/** Where something at unscrolled {@code y} shows now. */
	public int shown(int y) {
		return y - offset;
	}

	/** The bar on the right edge, when there's more than fits. */
	public void drawBar(Gfx g, Style s) {
		int max = maxOffset();
		if (max == 0) {
			return;
		}
		int thumb = thumbHeight();
		int top = thumbTop();
		int x = box[2] + 2;
		Draw.round(g, x, box[1], x + BAR_W, box[3], 1, Draw.alpha(s.border, 0.5f));
		Draw.round(g, x, top, x + BAR_W, top + thumb, 1, s.muted);
	}
}
