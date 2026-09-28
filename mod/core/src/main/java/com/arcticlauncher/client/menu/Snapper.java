package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.List;

/**
 * Snapping for dragged HUD widgets: to the screen's edges and center and to
 * other widgets (edges lined up, or stacked beside them), with guide lines.
 */
final class Snapper {
	private static final int SNAP = 5;

	private final List<Integer> guidesX = new ArrayList<Integer>();
	private final List<Integer> guidesY = new ArrayList<Integer>();

	void clear() {
		guidesX.clear();
		guidesY.clear();
	}

	/** The snapped left edge for a widget {@code w} wide. */
	int x(int x, int w, int screen, List<int[]> others) {
		List<int[]> c = new ArrayList<int[]>(); // {left, guide}
		c.add(new int[] {Hud.MARGIN, Hud.MARGIN});
		c.add(new int[] {screen - Hud.MARGIN - w, screen - Hud.MARGIN});
		c.add(new int[] {(screen - w) / 2, screen / 2});
		for (int[] o : others) {
			c.add(new int[] {o[0], o[0]});
			c.add(new int[] {o[0] + o[2] - w, o[0] + o[2]});
			c.add(new int[] {o[0] + o[2] + Hud.GAP, o[0] + o[2]});
			c.add(new int[] {o[0] - w - Hud.GAP, o[0]});
		}
		return snap(x, c, guidesX);
	}

	/** The snapped top edge for a widget {@code h} tall. */
	int y(int y, int h, int screen, List<int[]> others) {
		List<int[]> c = new ArrayList<int[]>();
		c.add(new int[] {Hud.MARGIN, Hud.MARGIN});
		c.add(new int[] {screen - Hud.MARGIN - h, screen - Hud.MARGIN});
		c.add(new int[] {(screen - h) / 2, screen / 2});
		for (int[] o : others) {
			c.add(new int[] {o[1], o[1]});
			c.add(new int[] {o[1] + o[3] - h, o[1] + o[3]});
			c.add(new int[] {o[1] + o[3] + Hud.GAP, o[1] + o[3]});
			c.add(new int[] {o[1] - h - Hud.GAP, o[1]});
		}
		return snap(y, c, guidesY);
	}

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

	void draw(Gfx g, Style s, int width, int height) {
		int color = Draw.alpha(s.accent, 0.7f);
		for (int x : guidesX) {
			g.fill(x, 0, x + 1, height, color);
		}
		for (int y : guidesY) {
			g.fill(0, y, width, y + 1, color);
		}
	}
}
