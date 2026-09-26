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

/** Drag HUD widgets into place; scroll to resize, right-click to hide. */
public final class HudEditor extends Page {
	private static final int SNAP = 4;
	private static final float SCALE_STEP = 0.1f;
	private static final String HINT = "Drag to move  ·  Scroll to resize  ·  Right-click to hide";

	private HudWidget dragging;
	private int grabX;
	private int grabY;
	private int mouseX;
	private int mouseY;
	/** Rectangles from the last frame, for hit tests between frames. */
	private final java.util.Map<HudWidget, int[]> rects = new java.util.HashMap<HudWidget, int[]>();

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
		if (dragging != null) {
			placeDragged(g, hud);
		}
		rects.clear();
		for (HudWidget w : hud.widgets()) {
			if (!hud.slot(w).enabled) {
				continue;
			}
			int[] r = hud.rect(g, w);
			rects.put(w, r);
			boolean over = w == dragging || (dragging == null && inside(r, mx, my));
			Draw.round(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, over ? 0.22f : 0.08f));
			Draw.outline(g, r[0] - 2, r[1] - 2, r[0] + r[2] + 2, r[1] + r[3] + 2, 2, Draw.alpha(s.accent, over ? 0.9f : 0.4f));
			hud.draw(g, s, w, true);
		}
		Draw.centered(g, HINT, width / 2, height / 2 - 26, s.text, true);
	}

	/** Follow the mouse, snapping the widget's center to the screen's. */
	private void placeDragged(Gfx g, Hud hud) {
		int[] r = hud.rect(g, dragging);
		int x = mouseX - grabX;
		int y = mouseY - grabY;
		if (Math.abs(x + r[2] / 2 - width / 2) < SNAP) {
			x = width / 2 - r[2] / 2;
		}
		if (Math.abs(y + r[3] / 2 - height / 2) < SNAP) {
			y = height / 2 - r[3] / 2;
		}
		hud.move(g, dragging, x, y);
		if (x + r[2] / 2 == width / 2) {
			g.fill(width / 2, 0, width / 2 + 1, height, Draw.alpha(style().accent, 0.5f));
		}
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
