package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.waypoints.Waypoint;
import java.util.ArrayList;
import java.util.List;

/**
 * A map of the land around you ({@link MinimapView} draws it): north up or turning with you,
 * square or round; you as an arrow in the middle, other players as arrows, waypoints as dots,
 * pinned to the edge when further away.
 */
final class Minimap extends HudWidget {
	private static final int BORDER = 2;
	private static final int OTHER_PLAYER = 0xFFFFFFFF;

	private final MinimapView view = new MinimapView();

	Minimap() {
		super("minimap", "Minimap", "The land around you, with other players and your waypoints", false, Column.RIGHT);
	}

	private static int size() {
		return ArcticClient.config().minimapSize;
	}

	@Override
	public boolean needsGame() {
		return true;
	}

	@Override
	public int width(Gfx g) {
		return size() + BORDER * 2;
	}

	@Override
	public int height() {
		return size() + BORDER * 2 + (ArcticClient.config().minimapCoords ? 10 : 0);
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		Platform p = ArcticClient.platform();
		ClientConfig c = ArcticClient.config();
		int size = size();
		int outer = size + BORDER * 2;
		int mid = BORDER + size / 2;
		if (c.minimapRound) {
			// A ring: the round picture covers all of it but its edge.
			Draw.disc(g, mid, mid, size / 2 + 1, Draw.alpha(accent(), 0.9f));
		} else {
			panel(g, s, 0, 0, outer, outer);
		}
		double[] pos = p.position();
		String texture = pos == null ? null : picture(p, c, pos);
		if (texture != null) {
			g.texture(texture, BORDER, BORDER, size, size, 0, 0, MinimapView.PICTURE, MinimapView.PICTURE, MinimapView.PICTURE,
					MinimapView.PICTURE);
		} else if (c.minimapRound) {
			Draw.disc(g, mid, mid, size / 2, 0xFF2F5D3A);
		} else {
			Draw.round(g, BORDER, BORDER, BORDER + size, BORDER + size, 2, 0xFF2F5D3A);
		}
		north(g, c, pos == null ? 180 : pos[3], size);
		if (pos != null && c.minimapCoords) {
			String where = (int) Math.floor(pos[0]) + " " + (int) Math.floor(pos[2]);
			Draw.centered(g, where, outer / 2, outer + 1, text(), shadow());
		}
	}

	/** The picture for this frame (its texture), or null where there's none. */
	private String picture(Platform p, ClientConfig c, double[] pos) {
		view.update(p::mapColumns, pos[0], pos[2], System.nanoTime());
		List<MinimapView.Marker> markers = new ArrayList<MinimapView.Marker>();
		for (Waypoint w : ArcticClient.waypoints().here(p)) {
			if (w.shown) {
				markers.add(new MinimapView.Marker(w.x + 0.5, w.z + 0.5, Double.NaN, w.color, false));
			}
		}
		if (c.minimapPlayers) {
			for (Object[] other : p.otherPlayers()) {
				double yaw = other.length > 5 && other[5] instanceof Number ? ((Number) other[5]).doubleValue() : Double.NaN;
				markers.add(new MinimapView.Marker(((Number) other[2]).doubleValue(), ((Number) other[4]).doubleValue(), yaw,
						OTHER_PLAYER, false));
			}
		}
		view.compose(pos[0], pos[2], pos[3], c.minimapZoom, c.minimapRotate, c.minimapRound, markers, accent());
		return p.minimapPixels(view.pixels, MinimapView.PICTURE);
	}

	/** "N" on the edge, where north is (it goes round when the map turns with you). */
	private void north(Gfx g, ClientConfig c, double yaw, int size) {
		double angle = c.minimapRotate ? MinimapView.screenAngle(180, true, yaw) : 0;
		double r = size / 2.0 - 5;
		int cx = BORDER + size / 2;
		int cy = BORDER + size / 2;
		int x = (int) Math.round(cx + Math.sin(angle) * r);
		int y = (int) Math.round(cy - Math.cos(angle) * r);
		Draw.round(g, x - 5, y - 5, x + 5, y + 5, 5, 0xB0000000);
		Draw.centered(g, "N", x, y - 4, text(), false);
	}
}
