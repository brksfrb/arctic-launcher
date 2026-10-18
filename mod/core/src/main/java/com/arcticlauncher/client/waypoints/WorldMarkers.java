package com.arcticlauncher.client.waypoints;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * Waypoints drawn where they are in the world: a colored marker with the
 * name and distance, placed by projecting the waypoint through the camera.
 * Nearer ones are drawn last (on top); the one you look at shows its name
 * even among many.
 */
public final class WorldMarkers {
	/** Closer than this (blocks), the marker hides: you're there. */
	private static final double ARRIVED = 3.0;
	/** In front of the camera by at least this much (blocks). */
	private static final double NEAR_PLANE = 0.05;
	private static final int DIAMOND = 4;
	private static final int PAD = 3;
	/** Names show for markers within this many pixels of the crosshair (others: distance only). */
	private static final int FOCUS_RADIUS = 40;
	private static final int LABEL_BG = 0xA0101318;

	private WorldMarkers() {}

	/** A waypoint on screen: where, how far, and the waypoint. */
	private static final class Placed {
		final Waypoint w;
		final double x;
		final double y;
		final double distance;

		Placed(Waypoint w, double x, double y, double distance) {
			this.w = w;
			this.x = x;
			this.y = y;
			this.distance = distance;
		}
	}

	public static void render(Gfx g, Style s, Platform p, List<Waypoint> waypoints) {
		double[] cam = p.camera();
		if (cam == null || waypoints.isEmpty()) {
			return;
		}
		int w = g.width();
		int h = g.height();
		List<Placed> placed = new ArrayList<Placed>();
		for (Waypoint wp : waypoints) {
			if (!wp.shown) {
				continue;
			}
			double tx = wp.x + 0.5;
			double ty = wp.y + 1.0;
			double tz = wp.z + 0.5;
			double distance = Math.sqrt(sq(tx - cam[0]) + sq(ty - cam[1]) + sq(tz - cam[2]));
			if (distance < ARRIVED) {
				continue;
			}
			double[] at = project(cam, tx, ty, tz, w, h);
			if (at != null) {
				placed.add(new Placed(wp, at[0], at[1], distance));
			}
		}
		// Far first, so near ones end up on top.
		placed.sort((a, b) -> Double.compare(b.distance, a.distance));
		for (Placed pl : placed) {
			draw(g, s, pl, w, h);
		}
	}

	private static void draw(Gfx g, Style s, Placed placed, int w, int h) {
		// The whole marker is drawn at the whole GUI pixel and shifted by the rest: rounding the
		// position itself would make it jump a GUI pixel (several screen pixels at a big GUI
		// scale) at a time while you move.
		int px = (int) Math.floor(placed.x);
		int py = (int) Math.floor(placed.y);
		g.push();
		g.translate((float) (placed.x - px), (float) (placed.y - py));
		drawAt(g, s, placed, px, py, w, h);
		g.pop();
	}

	private static void drawAt(Gfx g, Style s, Placed pl, int px, int py, int w, int h) {
		int color = pl.w.color;
		Draw.round(g, px - DIAMOND, py - DIAMOND, px + DIAMOND, py + DIAMOND, DIAMOND, 0xFF000000);
		Draw.round(g, px - DIAMOND + 1, py - DIAMOND + 1, px + DIAMOND - 1, py + DIAMOND - 1, DIAMOND - 1, color);
		String distance = distanceText(pl.distance);
		boolean focused = Math.abs(pl.x - w / 2) < FOCUS_RADIUS && Math.abs(pl.y - h / 2) < FOCUS_RADIUS;
		String text = focused ? pl.w.name + "  " + distance : distance;
		int tw = g.textWidth(text);
		int x0 = px - tw / 2 - PAD;
		int y0 = py - DIAMOND - 14;
		Draw.round(g, x0, y0, x0 + tw + PAD * 2, y0 + 12, 3, LABEL_BG);
		if (focused) {
			int nameW = g.textWidth(pl.w.name + "  ");
			g.text(pl.w.name, x0 + PAD, y0 + 2, color, false);
			g.text(distance, x0 + PAD + nameW, y0 + 2, s.text, false);
		} else {
			g.text(distance, x0 + PAD, y0 + 2, s.text, false);
		}
	}

	static String distanceText(double d) {
		return d < 1000 ? Math.round(d) + "m" : String.format(Locale.ROOT, "%.1fkm", d / 1000.0);
	}

	/**
	 * Where a world point shows on a {@code w}×{@code h} screen (not rounded), or null
	 * when it's behind the camera. {@code cam} is {x, y, z, yaw, pitch,
	 * vertical fov}, angles in degrees the way Minecraft counts them.
	 */
	static double[] project(double[] cam, double px, double py, double pz, int w, int h) {
		double yaw = Math.toRadians(cam[3]);
		double pitch = Math.toRadians(cam[4]);
		// Minecraft's look vector, and the camera's right and up.
		double fx = -Math.sin(yaw) * Math.cos(pitch);
		double fy = -Math.sin(pitch);
		double fz = Math.cos(yaw) * Math.cos(pitch);
		double rx = -Math.cos(yaw);
		double rz = -Math.sin(yaw);
		// up = right × forward (right has no y).
		double ux = -rz * fy;
		double uy = rz * fx - rx * fz;
		double uz = rx * fy;
		double dx = px - cam[0];
		double dy = py - cam[1];
		double dz = pz - cam[2];
		double depth = dx * fx + dy * fy + dz * fz;
		if (depth < NEAR_PLANE) {
			return null;
		}
		double right = dx * rx + dz * rz;
		double up = dx * ux + dy * uy + dz * uz;
		double scale = (h / 2.0) / Math.tan(Math.toRadians(cam[5]) / 2.0);
		double sx = w / 2.0 + right / depth * scale;
		double sy = h / 2.0 - up / depth * scale;
		if (sx < -w || sx > 2 * w || sy < -h || sy > 2 * h) {
			return null;
		}
		return new double[] {sx, sy};
	}

	private static double sq(double v) {
		return v * v;
	}
}
