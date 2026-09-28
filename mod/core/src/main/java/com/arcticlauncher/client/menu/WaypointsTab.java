package com.arcticlauncher.client.menu;

import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.waypoints.Waypoint;
import com.arcticlauncher.client.waypoints.Waypoints;

/**
 * Waypoints: add one where you stand, rename, recolor, show or hide, or
 * remove this world's; they show in the world (and on the Compass).
 */
final class WaypointsTab implements MenuTab {
	private static final int FIELD_H = 18;
	private static final int ROW_H = 24;
	private static final int GAP = 4;
	private static final int ADD_W = 80;
	private static final int SWATCH = 12;
	private static final int NAME_W = 110;
	private static final int SHOW_W = 52;
	private static final int REMOVE_W = 18;
	private static final int MAX_NAME = 24;

	private final TextField newName = new TextField("Name (optional)", MAX_NAME, TextField.ANY);
	private int x;
	private int w;
	private int listTop;
	private boolean inWorld;
	private boolean none;

	@Override
	public String title() {
		return "Waypoints";
	}

	@Override
	public String hint() {
		return "Places in this world, shown where they are.";
	}

	@Override
	public boolean needsWorld() {
		return true;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.w = w;
		final Platform p = ArcticClient.platform();
		final Waypoints waypoints = ArcticClient.waypoints();
		final ClientConfig c = ArcticClient.config();
		inWorld = p.worldKey() != null;
		Form f = new Form(host, x, top, w, bottom);
		if (inWorld) {
			f.section("Add one here");
			int row = f.row(ROW_H);
			final Runnable add = () -> {
				Waypoint made = waypoints.add(p, newName.text());
				if (made == null) {
					com.arcticlauncher.client.notice.Notices.post("Couldn't add it", "This world has " + Waypoints.MAX_PER_WORLD + " already.");
				}
				newName.text("");
				host.rebuild();
			};
			newName.onEnter(add);
			f.put(newName, f.x, row + (ROW_H - FIELD_H) / 2, f.w - ADD_W - GAP, FIELD_H);
			f.put(new Button("Add here", add).primary(), f.x + f.w - ADD_W, row + (ROW_H - FIELD_H) / 2, ADD_W, FIELD_H);
			List<Waypoint> here = waypoints.here(p);
			none = here.isEmpty();
			f.section("In this world (" + here.size() + ")");
			listTop = f.y();
			double[] pos = p.position();
			for (final Waypoint wp : here) {
				waypointRow(host, f, waypoints, p, wp, pos);
			}
			if (none) {
				f.gap(ROW_H);
			}
		}
		f.section("Settings");
		f.toggle("Show in the world", "Markers with the name and how far, where they are",
				() -> c.waypointsInWorld, on -> c.waypointsInWorld = on);
		f.key("Waypoint key", "Drops one where you stand, named by number",
				Form.key(() -> c.waypointKey, k -> c.waypointKey = k));
		f.toggle("Death waypoint", "A \"Death\" waypoint (and a pop-up) where you die",
				() -> c.deathNotice, on -> c.deathNotice = on);
		f.note("The Compass HUD widget shows them too.");
		f.done();
	}

	private void waypointRow(final Host host, Form f, final Waypoints waypoints, final Platform p, final Waypoint wp, double[] pos) {
		int row = f.row(ROW_H);
		int mid = row + (ROW_H - FIELD_H) / 2;
		f.put(new Swatch(wp.color, false, () -> {
			wp.color = nextColor(wp.color);
			waypoints.save();
			host.rebuild();
		}), f.x + 2, row + (ROW_H - SWATCH) / 2, SWATCH, SWATCH);
		final TextField name = new TextField("Name", MAX_NAME, TextField.ANY).text(wp.name);
		name.onChange(() -> {
			if (!name.text().trim().isEmpty()) {
				wp.name = name.text().trim();
				waypoints.save();
			}
		});
		f.put(name, f.x + SWATCH + 8, mid, NAME_W, FIELD_H);
		String where = wp.x + " " + wp.y + " " + wp.z;
		if (pos != null) {
			where += String.format(Locale.ROOT, "  ·  %dm", Math.round(wp.distance(pos[0], pos[2])));
		}
		int infoX = f.x + SWATCH + 8 + NAME_W + 8;
		int infoW = f.w - (infoX - f.x) - SHOW_W - REMOVE_W - GAP * 2;
		f.put(new Label(where, Label.Kind.MUTED), infoX, row, infoW, ROW_H);
		f.put(new Button(wp.shown ? "Shown" : "Hidden", () -> {
			wp.shown = !wp.shown;
			waypoints.save();
			host.rebuild();
		}).selected(wp.shown), f.x + f.w - REMOVE_W - GAP - SHOW_W, mid, SHOW_W, FIELD_H);
		f.put(new Button("×", () -> {
			waypoints.remove(p, wp);
			host.rebuild();
		}).confirm("?"), f.x + f.w - REMOVE_W, mid, REMOVE_W, FIELD_H);
	}

	private static int nextColor(int color) {
		for (int i = 0; i < Waypoints.COLORS.length; i++) {
			if (Waypoints.COLORS[i] == color) {
				return Waypoints.COLORS[(i + 1) % Waypoints.COLORS.length];
			}
		}
		return Waypoints.COLORS[0];
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		if (!inWorld) {
			return;
		}
		if (none) {
			g.text(Draw.fit(g, "None yet: name one and press Add here, or set the key below.", w), x + 4, listTop + 8, s.muted, false);
		}
	}

	@Override
	public String state() {
		Platform p = ArcticClient.platform();
		return p.worldKey() + "/" + p.dimension() + "/" + ArcticClient.waypoints().here(p).size();
	}
}
