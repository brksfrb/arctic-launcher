package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Date;
import java.util.List;

/** The HUD widgets, where they sit, and drawing them. */
public final class Hud {
	public static final int MARGIN = 4;
	public static final float MIN_SCALE = 0.5f;
	public static final float MAX_SCALE = 2.5f;
	private static final String[] COMPASS = {"S", "SW", "W", "NW", "N", "NE", "E", "SE"};

	private final ClientConfig config;
	private final Cps cps = new Cps();
	private final List<HudWidget> widgets;

	public Hud(ClientConfig config) {
		this.config = config;
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(fps());
		all.add(cpsWidget());
		all.add(ping());
		all.add(coords());
		all.add(clock());
		all.add(new Keystrokes(cps));
		widgets = Collections.unmodifiableList(all);
	}

	public List<HudWidget> widgets() {
		return widgets;
	}

	public Cps cps() {
		return cps;
	}

	/** The widget's placement, created from its defaults on first use. */
	public HudSlot slot(HudWidget w) {
		HudSlot slot = config.hud.get(w.id);
		if (slot == null) {
			slot = w.defaults.copy();
			config.hud.put(w.id, slot);
		}
		return slot;
	}

	public void reset() {
		config.hud.clear();
	}

	/** Screen rectangle {x, y, w, h} of a widget. */
	public int[] rect(Gfx g, HudWidget w) {
		HudSlot slot = slot(w);
		int ww = Math.round(w.width(g) * slot.scale);
		int hh = Math.round(w.height() * slot.scale);
		int x = MARGIN + Math.round(clamp(slot.x) * Math.max(0, g.width() - 2 * MARGIN - ww));
		int y = MARGIN + Math.round(clamp(slot.y) * Math.max(0, g.height() - 2 * MARGIN - hh));
		return new int[] {x, y, ww, hh};
	}

	/** Place a widget so its top-left is at (x, y). */
	public void move(Gfx g, HudWidget w, int x, int y) {
		HudSlot slot = slot(w);
		int ww = Math.round(w.width(g) * slot.scale);
		int hh = Math.round(w.height() * slot.scale);
		int freeX = g.width() - 2 * MARGIN - ww;
		int freeY = g.height() - 2 * MARGIN - hh;
		slot.x = freeX <= 0 ? 0 : clamp((x - MARGIN) / (float) freeX);
		slot.y = freeY <= 0 ? 0 : clamp((y - MARGIN) / (float) freeY);
	}

	private static float clamp(float v) {
		return Math.max(0f, Math.min(1f, v));
	}

	public void render(Gfx g, Style s, boolean preview) {
		for (HudWidget w : widgets) {
			if (slot(w).enabled) {
				draw(g, s, w, preview);
			}
		}
	}

	public void draw(Gfx g, Style s, HudWidget w, boolean preview) {
		int[] r = rect(g, w);
		g.push();
		g.translate(r[0], r[1]);
		g.scale(slot(w).scale);
		w.render(g, s, preview);
		g.pop();
	}

	// ---- Built-in widgets --------------------------------------------------

	private static Platform platform() {
		return ArcticClient.platform();
	}

	private static HudWidget fps() {
		return new TextWidget("fps", "FPS", "Frames per second", "000", new HudSlot(true, 0f, 0f)) {
			@Override
			protected String value(boolean preview) {
				return String.valueOf(platform().fps());
			}
		};
	}

	private HudWidget cpsWidget() {
		return new TextWidget("cps", "CPS", "Clicks per second (left | right)", "00 | 00", new HudSlot(true, 0f, 0.07f)) {
			@Override
			protected String value(boolean preview) {
				return cps.get(Keys.MOUSE_LEFT) + " | " + cps.get(Keys.MOUSE_RIGHT);
			}
		};
	}

	private static HudWidget ping() {
		return new TextWidget("ping", "Ping", "Latency to the server", "000 ms", new HudSlot(true, 0f, 0.14f)) {
			@Override
			protected String value(boolean preview) {
				int ms = platform().ping();
				if (ms < 0) {
					return preview ? "24 ms" : "--";
				}
				return ms + " ms";
			}
		};
	}

	private static HudWidget coords() {
		return new TextWidget("coords", "XYZ", "Coordinates and facing", "-0000 000 -0000 NW", new HudSlot(false, 0f, 0.21f)) {
			@Override
			protected String value(boolean preview) {
				double[] p = platform().position();
				if (p == null) {
					return "0 64 0 N";
				}
				return (int) Math.floor(p[0]) + " " + (int) Math.floor(p[1]) + " " + (int) Math.floor(p[2]) + " " + facing(p[3]);
			}
		};
	}

	/** Compass direction for a Minecraft yaw (0 = south). */
	static String facing(double yaw) {
		double wrapped = ((yaw % 360) + 360) % 360;
		int index = (int) Math.round(wrapped / 45.0) % COMPASS.length;
		return COMPASS[index];
	}

	private static HudWidget clock() {
		return new TextWidget("clock", "Time", "Your local time", "00:00", new HudSlot(false, 0f, 0.28f)) {
			private final SimpleDateFormat format = new SimpleDateFormat("HH:mm");

			@Override
			protected String value(boolean preview) {
				return format.format(new Date());
			}
		};
	}
}
