package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Toggle;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * The HUD editor over the real game: a drawer lists every widget (switch
 * it on and it slots into place) and edits the selected one; widgets on
 * screen drag and snap, and dropping one on the drawer hides it.
 */
public final class HudEditor extends Page {
	private static final int SNAP = 5;
	private static final float SCALE_STEP = 0.1f;
	private static final int DRAWER_W = 150;
	private static final int PAD = 6;
	private static final int ROW = 24;
	private static final int FOOTER = 28;
	/** How far the mouse moves before a click becomes a drag. */
	private static final int DRAG_START = 3;

	private static boolean drawerOpen = true;
	private int scroll;
	private HudWidget selected;
	private HudWidget dragging;
	private boolean moved;
	private int pressX;
	private int pressY;
	private int grabX;
	private int grabY;
	private int mouseX;
	private int mouseY;
	/** Rectangles from the last frame, for hit tests between frames. */
	private Map<HudWidget, int[]> rects = new java.util.HashMap<HudWidget, int[]>();
	private final List<Integer> guidesX = new ArrayList<Integer>();
	private final List<Integer> guidesY = new ArrayList<Integer>();
	private int listTop;
	private int listBottom;

	@Override
	public boolean dimWorld() {
		return false;
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void build() {
		if (!drawerOpen) {
			add(new Button(">", () -> {
				drawerOpen = true;
				rebuild();
			})).bounds(0, height / 2 - 14, 12, 28);
			return;
		}
		add(new Button("<", () -> {
			drawerOpen = false;
			rebuild();
		})).bounds(DRAWER_W - 18, 6, 14, 14);
		int y = 24;
		if (selected != null && ArcticClient.hud().slot(selected).enabled) {
			y = buildSelected(y);
		}
		listTop = y + 18;
		listBottom = height - FOOTER;
		buildList();
		int half = (DRAWER_W - PAD * 2 - 4) / 2;
		add(new Button("Reset all", () -> {
			ArcticClient.hud().reset();
			selected = null;
			rebuild();
		})).bounds(PAD, height - FOOTER + 5, half, 18);
		add(new Button("Done", this::close).primary()).bounds(PAD + half + 4, height - FOOTER + 5, half, 18);
	}

	/** Size, background, reset and hide for the selected widget. */
	private int buildSelected(int y) {
		final HudSlot slot = ArcticClient.hud().slot(selected);
		int x = PAD;
		int w = DRAWER_W - PAD * 2;
		y += 12;
		add(new Button("-", () -> resize(slot, -SCALE_STEP))).bounds(x + w - 58, y, 16, 16);
		add(new Button("+", () -> resize(slot, SCALE_STEP))).bounds(x + w - 16, y, 16, 16);
		y += 20;
		add(new Toggle("Background", null, new Toggle.Binding() {
			@Override
			public boolean get() {
				return slot.background;
			}

			@Override
			public void set(boolean on) {
				slot.background = on;
				ArcticClient.saveConfig();
			}
		})).bounds(x, y, w, 18);
		y += 22;
		int half = (w - 4) / 2;
		add(new Button("Reset spot", () -> {
			slot.placed = false;
			ArcticClient.saveConfig();
		})).bounds(x, y, half, 16);
		add(new Button("Hide", () -> {
			slot.enabled = false;
			slot.placed = false;
			selected = null;
			ArcticClient.saveConfig();
			rebuild();
		})).bounds(x + half + 4, y, w - half - 4, 16);
		return y + 20;
	}

	private void resize(HudSlot slot, float step) {
		float next = Math.round((slot.scale + step) * 10) / 10f;
		slot.scale = Math.max(Hud.MIN_SCALE, Math.min(Hud.MAX_SCALE, next));
		ArcticClient.saveConfig();
	}

	/** Every widget with its description and switch, scrolled. */
	private void buildList() {
		List<HudWidget> all = ArcticClient.hud().widgets();
		int visible = Math.max(1, (listBottom - listTop) / ROW);
		scroll = Math.max(0, Math.min(scroll, Math.max(0, all.size() - visible)));
		int y = listTop;
		for (int i = scroll; i < all.size() && y + ROW <= listBottom; i++) {
			final HudWidget w = all.get(i);
			final HudSlot slot = ArcticClient.hud().slot(w);
			add(new Toggle(w.name, w.description, new Toggle.Binding() {
				@Override
				public boolean get() {
					return slot.enabled;
				}

				@Override
				public void set(boolean on) {
					slot.enabled = on;
					if (on) {
						slot.placed = false; // slot it into its column
						selected = w;
					} else if (selected == w) {
						selected = null;
					}
					ArcticClient.saveConfig();
					rebuild();
				}
			})).bounds(PAD - 2, y, DRAWER_W - PAD * 2 + 4, ROW - 2);
			y += ROW;
		}
	}

	private boolean inDrawer(double mx) {
		return drawerOpen && mx < DRAWER_W;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Hud hud = ArcticClient.hud();
		guidesX.clear();
		guidesY.clear();
		if (dragging != null && moved) {
			placeDragged(g, hud);
		}
		rects = hud.layout(g);
		for (Map.Entry<HudWidget, int[]> e : hud.sorted(rects)) {
			drawWidget(g, s, hud, e.getKey(), e.getValue(), mx, my);
		}
		int guide = Draw.alpha(s.accent, 0.7f);
		for (int x : guidesX) {
			g.fill(x, 0, x + 1, height, guide);
		}
		for (int y : guidesY) {
			g.fill(0, y, width, y + 1, guide);
		}
		if (drawerOpen) {
			drawDrawer(g, s, mx);
		}
	}

	private void drawWidget(Gfx g, Style s, Hud hud, HudWidget w, int[] r, int mx, int my) {
		boolean over = w == dragging || (dragging == null && !inDrawer(mx) && inside(r, mx, my));
		boolean chosen = w == selected;
		float fill = chosen ? 0.25f : over ? 0.18f : 0.06f;
		float line = chosen || over ? 0.95f : 0.35f;
		Draw.round(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, fill));
		Draw.outline(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, line));
		hud.draw(g, s, w, r, true);
	}

	private void drawDrawer(Gfx g, Style s, int mx) {
		boolean dropping = dragging != null && moved && inDrawer(mx);
		// Solid, so widgets underneath don't show through the list.
		Draw.round(g, -4, -4, DRAWER_W, height + 4, 3, 0xFF000000 | (s.panel & 0xFFFFFF));
		Draw.outline(g, -4, -4, DRAWER_W, height + 4, 3, s.border);
		if (dropping) {
			Draw.round(g, 2, 2, DRAWER_W - 2, height - 2, 3, Draw.alpha(s.accent, 0.18f));
			Draw.centered(g, "Drop here to hide", DRAWER_W / 2, height / 2, s.accent, false);
			return;
		}
		g.text("§lHUD", PAD, 9, s.text, false);
		if (selected != null && ArcticClient.hud().slot(selected).enabled) {
			HudSlot slot = ArcticClient.hud().slot(selected);
			g.text(Draw.fit(g, selected.name, DRAWER_W - PAD * 2), PAD, 26, s.accent, false);
			g.text("Size", PAD + 2, 40, s.text, false);
			Draw.centered(g, Math.round(slot.scale * 100) + "%", PAD + (DRAWER_W - PAD * 2) - 29, 40, s.text, false);
		} else {
			g.text(Draw.fit(g, "Click a widget to change it", DRAWER_W - PAD * 2), PAD, 26, s.muted, false);
		}
		g.fill(PAD, listTop - 4, DRAWER_W - PAD, listTop - 3, s.border);
		List<HudWidget> all = ArcticClient.hud().widgets();
		int visible = Math.max(1, (listBottom - listTop) / ROW);
		if (all.size() > visible) {
			// A thin scroll bar beside the list.
			int track = listBottom - listTop;
			int bar = Math.max(12, track * visible / all.size());
			int top = listTop + (track - bar) * scroll / Math.max(1, all.size() - visible);
			g.fill(DRAWER_W - 3, top, DRAWER_W - 1, top + bar, Draw.alpha(s.accent, 0.6f));
		}
		g.fill(PAD, height - FOOTER, DRAWER_W - PAD, height - FOOTER + 1, s.border);
	}

	/** Follow the mouse, snapping to the screen and the other widgets. */
	private void placeDragged(Gfx g, Hud hud) {
		int[] me = rects.get(dragging);
		if (me == null) {
			return;
		}
		int x = mouseX - grabX;
		int y = mouseY - grabY;
		List<int[]> others = new ArrayList<int[]>();
		for (Map.Entry<HudWidget, int[]> e : rects.entrySet()) {
			if (e.getKey() != dragging) {
				others.add(e.getValue());
			}
		}
		x = snapX(x, me[2], others);
		y = snapY(y, me[3], others);
		hud.place(g, dragging, me, x, y);
	}

	/** Candidate left edges: screen edges and center, and other widgets' edges. */
	private int snapX(int x, int w, List<int[]> others) {
		List<int[]> c = new ArrayList<int[]>(); // {left, guide}
		c.add(new int[] {Hud.MARGIN, Hud.MARGIN});
		c.add(new int[] {width - Hud.MARGIN - w, width - Hud.MARGIN});
		c.add(new int[] {(width - w) / 2, width / 2});
		for (int[] o : others) {
			c.add(new int[] {o[0], o[0]});
			c.add(new int[] {o[0] + o[2] - w, o[0] + o[2]});
			c.add(new int[] {o[0] + o[2] + Hud.GAP, o[0] + o[2]});
			c.add(new int[] {o[0] - w - Hud.GAP, o[0]});
		}
		return snap(x, c, guidesX);
	}

	private int snapY(int y, int h, List<int[]> others) {
		List<int[]> c = new ArrayList<int[]>();
		c.add(new int[] {Hud.MARGIN, Hud.MARGIN});
		c.add(new int[] {height - Hud.MARGIN - h, height - Hud.MARGIN});
		c.add(new int[] {(height - h) / 2, height / 2});
		for (int[] o : others) {
			c.add(new int[] {o[1], o[1]});
			c.add(new int[] {o[1] + o[3] - h, o[1] + o[3]});
			c.add(new int[] {o[1] + o[3] + Hud.GAP, o[1] + o[3]});
			c.add(new int[] {o[1] - h - Hud.GAP, o[1]});
		}
		return snap(y, c, guidesY);
	}

	/** The closest candidate within reach, recording its guide line. */
	private static int snap(int pos, List<int[]> candidates, List<Integer> guides) {
		int best = pos;
		int bestDist = SNAP + 1;
		int guide = -1;
		for (int[] c : candidates) {
			int d = Math.abs(c[0] - pos);
			if (d < bestDist) {
				best = c[0];
				bestDist = d;
				guide = c[1];
			}
		}
		if (guide >= 0) {
			guides.add(guide);
		}
		return best;
	}

	private static boolean inside(int[] r, double mx, double my) {
		return mx >= r[0] && my >= r[1] && mx < r[0] + r[2] && my < r[1] + r[3];
	}

	private HudWidget widgetAt(double mx, double my) {
		if (inDrawer(mx)) {
			return null;
		}
		HudWidget found = null;
		for (HudWidget w : ArcticClient.hud().widgets()) {
			int[] r = rects.get(w);
			if (r != null && inside(r, mx, my)) {
				found = w;
			}
		}
		return found;
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (super.mouseClicked(mx, my, button)) {
			return true;
		}
		HudWidget w = widgetAt(mx, my);
		if (w == null) {
			if (!inDrawer(mx) && selected != null) {
				selected = null;
				rebuild();
			}
			return false;
		}
		if (button == Keys.MOUSE_RIGHT) {
			hide(w);
			return true;
		}
		int[] r = rects.get(w);
		dragging = w;
		moved = false;
		pressX = (int) mx;
		pressY = (int) my;
		grabX = (int) mx - r[0];
		grabY = (int) my - r[1];
		mouseX = (int) mx;
		mouseY = (int) my;
		if (selected != w) {
			selected = w;
			rebuild();
		}
		return true;
	}

	private void hide(HudWidget w) {
		HudSlot slot = ArcticClient.hud().slot(w);
		slot.enabled = false;
		slot.placed = false;
		if (selected == w) {
			selected = null;
		}
		ArcticClient.saveConfig();
		rebuild();
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button) {
		if (dragging == null) {
			return super.mouseDragged(mx, my, button);
		}
		mouseX = (int) mx;
		mouseY = (int) my;
		if (Math.abs(mx - pressX) > DRAG_START || Math.abs(my - pressY) > DRAG_START) {
			moved = true;
		}
		return true;
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (dragging == null) {
			return super.mouseReleased(mx, my, button);
		}
		HudWidget w = dragging;
		boolean dropped = moved && inDrawer(mx);
		dragging = null;
		moved = false;
		if (dropped) {
			hide(w);
		} else {
			ArcticClient.saveConfig();
		}
		return true;
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		if (inDrawer(mx)) {
			scroll += amount > 0 ? -1 : 1;
			rebuild();
			return true;
		}
		HudWidget w = widgetAt(mx, my);
		if (w == null) {
			return false;
		}
		resize(ArcticClient.hud().slot(w), amount > 0 ? SCALE_STEP : -SCALE_STEP);
		return true;
	}

	@Override
	public void close() {
		ArcticClient.saveConfig();
		super.close();
	}
}
