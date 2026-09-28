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

	/** A window from (x0, y0) to (x1, y1); starts scrolled by {@code offset}. */
	public Scroll(int x0, int y0, int x1, int y1, int offset) {
		box = new int[] {x0, y0, x1, y1};
		this.offset = Math.max(0, offset);
		contentBottom = y0;
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
		int height = box[3] - box[1];
		int total = height + max;
		int thumb = Math.max(MIN_THUMB, height * height / total);
		int top = box[1] + (int) ((long) (height - thumb) * offset / max);
		int x = box[2] + 2;
		Draw.round(g, x, box[1], x + BAR_W, box[3], 1, Draw.alpha(s.border, 0.5f));
		Draw.round(g, x, top, x + BAR_W, top + thumb, 1, s.muted);
	}
}
