package com.arcticlauncher.client.hud;

import java.util.Arrays;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.waypoints.Waypoint;

/**
 * A compass strip: the directions ahead of you, and your waypoints as
 * dots with their distance. The middle is where you look.
 */
final class Compass extends HudWidget {
	private static final int WIDTH = 180;
	private static final int HEIGHT = 24;
	/** Degrees shown to each side of the middle. */
	private static final double HALF_VIEW = 90;
	private static final String[] NAMES = {"S", "SW", "W", "NW", "N", "NE", "E", "SE"};
	private static final int DOT = 3;
	private static final int EDGE = 6;

	Compass() {
		super("compass", "Compass", "Directions ahead, with your waypoints and how far they are", false, Column.LEFT);
	}

	@Override
	public int width(Gfx g) {
		return WIDTH;
	}

	@Override
	public int height() {
		return HEIGHT;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		panel(g, s, 0, 0, WIDTH, HEIGHT);
		double[] p = ArcticClient.platform().position();
		double yaw = p == null ? 180 : p[3];
		int mid = WIDTH / 2;
		for (int i = 0; i < NAMES.length; i++) {
			int x = xFor(i * 45.0, yaw);
			if (x >= 0) {
				boolean main = i % 2 == 0;
				String name = NAMES[i];
				Draw.centered(g, name, x, 3, main ? text() : muted(), shadow());
			}
		}
		g.fill(mid, 1, mid + 1, 3, accent());
		List<Waypoint> list = preview || p == null ? samples() : ArcticClient.waypoints().here(ArcticClient.platform());
		double px = preview || p == null ? 0 : p[0];
		double pz = preview || p == null ? 0 : p[2];
		// Nearest first: it gets its label; ones that would overlap show just their dot.
		final double fx = px;
		final double fz = pz;
		List<Waypoint> sorted = new java.util.ArrayList<Waypoint>(list);
		sorted.sort((a, b) -> Double.compare(a.distance(fx, fz), b.distance(fx, fz)));
		List<int[]> taken = new java.util.ArrayList<int[]>();
		for (Waypoint w : sorted) {
			if (!w.shown) {
				continue;
			}
			int x = xFor(w.yawFrom(px, pz), yaw);
			if (x < 0) {
				continue;
			}
			Draw.round(g, x - DOT, 13, x + DOT, 13 + DOT * 2, DOT, w.color);
			int d = (int) Math.round(w.distance(px, pz));
			String label = d < 1000 ? d + "m" : Num.fixed(d / 1000.0, 1) + "k";
			int from = x + DOT + 2;
			int to = from + g.textWidth(label);
			boolean free = to <= WIDTH;
			for (int[] t : taken) {
				free &= to < t[0] || from > t[1];
			}
			if (free) {
				g.text(label, from, 13, w.color, shadow());
				taken.add(new int[] {x - DOT, to});
			}
		}
	}

	/** X on the strip for a direction, or -1 when it's behind you. */
	private static int xFor(double angle, double yaw) {
		double diff = ((angle - yaw) % 360 + 540) % 360 - 180;
		if (Math.abs(diff) > HALF_VIEW) {
			return -1;
		}
		int x = (int) Math.round(WIDTH / 2.0 + diff / HALF_VIEW * (WIDTH / 2.0 - EDGE));
		return Math.max(EDGE, Math.min(WIDTH - EDGE, x));
	}

	private static List<Waypoint> samples() {
		Waypoint home = new Waypoint();
		home.name = "Home";
		home.x = -40;
		home.z = 120;
		Waypoint mine = new Waypoint();
		mine.name = "Mine";
		mine.x = 300;
		mine.z = 200;
		mine.color = 0xFFFDE047;
		return Arrays.asList(home, mine);
	}
}
