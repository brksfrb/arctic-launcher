package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.waypoints.Waypoint;

/**
 * A north-up map of the blocks around you: you in the middle (with the way
 * you face), your waypoints as dots, pinned to the edge when further away.
 */
final class Minimap extends HudWidget {
	private static final int BLOCKS = 64;
	private static final int SIZE = 96;
	private static final int BORDER = 2;
	private static final int DOT = 2;
	private static final int ARROW = 4;

	Minimap() {
		super("minimap", "Minimap", "The land around you, north up, with your waypoints", false, Column.RIGHT);
	}

	@Override
	public boolean needsGame() {
		return true;
	}

	@Override
	public int width(Gfx g) {
		return SIZE + BORDER * 2;
	}

	@Override
	public int height() {
		return SIZE + BORDER * 2 + 10;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		Platform p = ArcticClient.platform();
		panel(g, s, 0, 0, SIZE + BORDER * 2, SIZE + BORDER * 2);
		String texture = p.minimap();
		if (texture != null) {
			g.texture(texture, BORDER, BORDER, SIZE, SIZE, 0, 0, BLOCKS, BLOCKS, BLOCKS, BLOCKS);
		} else {
			Draw.round(g, BORDER, BORDER, BORDER + SIZE, BORDER + SIZE, 2, 0xFF2F5D3A);
		}
		double[] pos = p.position();
		int mid = BORDER + SIZE / 2;
		if (pos != null && !preview) {
			double scale = SIZE / (double) BLOCKS;
			for (Waypoint w : ArcticClient.waypoints().here(p)) {
				if (!w.shown) {
					continue;
				}
				double dx = (w.x + 0.5 - pos[0]) * scale;
				double dz = (w.z + 0.5 - pos[2]) * scale;
				double far = Math.max(Math.abs(dx), Math.abs(dz)) / (SIZE / 2.0 - DOT);
				if (far > 1) {
					dx /= far;
					dz /= far;
				}
				int x = (int) Math.round(mid + dx);
				int y = (int) Math.round(mid + dz);
				Draw.round(g, x - DOT, y - DOT, x + DOT, y + DOT, DOT, w.color);
			}
		}
		// You: a dot and a tick the way you face (yaw 0 = south = down).
		double yaw = Math.toRadians(pos == null ? 0 : pos[3]);
		int tx = (int) Math.round(mid - Math.sin(yaw) * ARROW * 2);
		int ty = (int) Math.round(mid + Math.cos(yaw) * ARROW * 2);
		line(g, mid, mid, tx, ty, 0xFFFFFFFF);
		Draw.round(g, mid - ARROW / 2, mid - ARROW / 2, mid + ARROW / 2, mid + ARROW / 2, ARROW / 2, 0xFFFFFFFF);
		Draw.centered(g, "N", mid, BORDER + 2, text(), true);
		if (pos != null) {
			String where = (int) Math.floor(pos[0]) + " " + (int) Math.floor(pos[2]);
			Draw.centered(g, where, mid, SIZE + BORDER * 2 + 1, text(), shadow());
		}
	}

	/** A short line made of small squares. */
	private static void line(Gfx g, int x0, int y0, int x1, int y1, int color) {
		int steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
		for (int i = 0; i <= steps; i++) {
			int x = x0 + (x1 - x0) * i / Math.max(1, steps);
			int y = y0 + (y1 - y0) * i / Math.max(1, steps);
			g.fill(x, y, x + 1, y + 1, color);
		}
	}
}
