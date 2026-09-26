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
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Drag HUD widgets into place (they snap to edges, the center and each
 * other); scroll to resize, right-click to hide.
 */
public final class HudEditor extends Page {
	private static final int SNAP = 5;
	private static final float SCALE_STEP = 0.1f;
	private static final String HINT = "Drag to move  ·  Scroll to resize  ·  Right-click to hide";

	private HudWidget dragging;
	private int grabX;
	private int grabY;
	private int mouseX;
	private int mouseY;
	/** Rectangles from the last frame, for hit tests between frames. */
	private Map<HudWidget, int[]> rects = new java.util.HashMap<HudWidget, int[]>();
	/** Guide lines of the current snap: {x} verticals and {y} horizontals. */
	private final List<Integer> guidesX = new ArrayList<Integer>();
	private final List<Integer> guidesY = new ArrayList<Integer>();

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
		int bw = 80;
		int y = height / 2 - 10;
		add(new Button("Reset", new Runnable() {
			@Override
			public void run() {
				ArcticClient.hud().reset();
			}
		})).bounds(width / 2 - bw - 2, y, bw, 20);
		add(new Button("Done", new Runnable() {
			@Override
			public void run() {
				close();
			}
		}).primary()).bounds(width / 2 + 2, y, bw, 20);
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Hud hud = ArcticClient.hud();
		guidesX.clear();
		guidesY.clear();
		if (dragging != null) {
			placeDragged(g, hud);
		}
		rects = hud.layout(g);
		for (Map.Entry<HudWidget, int[]> e : hud.sorted(rects)) {
			HudWidget w = e.getKey();
			int[] r = e.getValue();
			boolean over = w == dragging || (dragging == null && inside(r, mx, my));
			Draw.round(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, over ? 0.22f : 0.08f));
			Draw.outline(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, over ? 0.9f : 0.4f));
			hud.draw(g, s, w, r, true);
		}
		int guide = Draw.alpha(s.accent, 0.7f);
		for (int x : guidesX) {
			g.fill(x, 0, x + 1, height, guide);
		}
		for (int y : guidesY) {
			g.fill(0, y, width, y + 1, guide);
		}
		Draw.centered(g, HINT, width / 2, height / 2 - 26, s.text, true);
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
			c.add(new int[] {o[0], o[0]});                   // left edges line up
			c.add(new int[] {o[0] + o[2] - w, o[0] + o[2]}); // right edges line up
			c.add(new int[] {o[0] + o[2] + Hud.GAP, o[0] + o[2]}); // just right of it
			c.add(new int[] {o[0] - w - Hud.GAP, o[0]});     // just left of it
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
			c.add(new int[] {o[1] + o[3] + Hud.GAP, o[1] + o[3]}); // stacked below
			c.add(new int[] {o[1] - h - Hud.GAP, o[1]});           // stacked above
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
			return false;
		}
		if (button == Keys.MOUSE_RIGHT) {
			ArcticClient.hud().slot(w).enabled = false;
			return true;
		}
		int[] r = rects.get(w);
		dragging = w;
		grabX = (int) mx - r[0];
		grabY = (int) my - r[1];
		mouseX = (int) mx;
		mouseY = (int) my;
		return true;
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button) {
		if (dragging == null) {
			return super.mouseDragged(mx, my, button);
		}
		mouseX = (int) mx;
		mouseY = (int) my;
		return true;
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (dragging == null) {
			return super.mouseReleased(mx, my, button);
		}
		dragging = null;
		ArcticClient.saveConfig();
		return true;
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		HudWidget w = widgetAt(mx, my);
		if (w == null) {
			return false;
		}
		HudSlot slot = ArcticClient.hud().slot(w);
		float next = slot.scale + (amount > 0 ? SCALE_STEP : -SCALE_STEP);
		slot.scale = Math.max(Hud.MIN_SCALE, Math.min(Hud.MAX_SCALE, Math.round(next * 10) / 10f));
		return true;
	}

	@Override
	public void close() {
		ArcticClient.saveConfig();
		super.close();
	}
}
