package com.arcticlauncher.client.hud;

import java.io.File;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.CrosshairConfig;
import com.arcticlauncher.client.gfx.Gfx;

/**
 * Draws a custom crosshair: cross, dot, circle, cross with a dot, or your
 * own picture; optionally coloured by what you aim at.
 */
public final class Crosshair {
	private static final int OUTLINE = 0xA0000000;

	private Crosshair() {}

	/** Biggest a picture crosshair is drawn (GUI pixels). */
	private static final int MAX_IMAGE = 32;
	/** Check the picture file this often. */
	private static final long IMAGE_CHECK_MS = 2000;

	private static long imageCheckedAt;
	private static long imageModified;
	private static String imageKey;
	private static int imageW;
	private static int imageH;

	/** Draw centered at (cx, cy). */
	public static void render(Gfx g, CrosshairConfig c, int cx, int cy) {
		int color = color(c);
		if ("image".equals(c.style) && drawImage(g, cx, cy)) {
			return;
		}
		// A picture that isn't there (yet): the cross instead.
		String style = "image".equals(c.style) ? "cross" : c.style;
		if (c.outline) {
			draw(g, c, style, cx, cy, 1, OUTLINE);
		}
		draw(g, c, style, cx, cy, 0, color);
	}

	/** The colour for what you're aiming at (or the usual one). */
	private static int color(CrosshairConfig c) {
		if (!c.targetColors || ArcticClient.platform() == null) {
			return c.color;
		}
		switch (ArcticClient.platform().aimKind()) {
			case 1:
				return c.playerColor;
			case 2:
				return c.hostileColor;
			case 3:
				return c.passiveColor;
			default:
				return c.color;
		}
	}

	/** The player's own picture, reloaded when the file changes; false if none. */
	private static boolean drawImage(Gfx g, int cx, int cy) {
		long now = System.currentTimeMillis();
		if (now - imageCheckedAt > IMAGE_CHECK_MS) {
			imageCheckedAt = now;
			File file = imageFile();
			long modified = file == null ? 0 : file.lastModified();
			if (modified != imageModified) {
				imageModified = modified;
				imageKey = null;
				load(file, modified);
			}
		}
		String key = imageKey;
		if (key == null || !ArcticClient.looks().isReady(key.substring(5))) {
			return false;
		}
		float scale = Math.min(1f, MAX_IMAGE / (float) Math.max(imageW, imageH));
		int w = Math.max(1, Math.round(imageW * scale));
		int h = Math.max(1, Math.round(imageH * scale));
		g.texture(key, cx - w / 2, cy - h / 2, w, h, 0, 0, imageW, imageH, imageW, imageH);
		return true;
	}

	public static File imageFile() {
		if (ArcticClient.platform() == null) {
			return null;
		}
		return new File(ArcticClient.platform().configDir(), CrosshairConfig.IMAGE_FILE);
	}

	private static void load(File file, long modified) {
		if (file == null || modified == 0) {
			return;
		}
		try {
			byte[] png = java.nio.file.Files.readAllBytes(file.toPath());
			if (png.length < 24 || png.length > 512 * 1024) {
				return;
			}
			imageW = ((png[16] & 0xFF) << 24) | ((png[17] & 0xFF) << 16) | ((png[18] & 0xFF) << 8) | (png[19] & 0xFF);
			imageH = ((png[20] & 0xFF) << 24) | ((png[21] & 0xFF) << 16) | ((png[22] & 0xFF) << 8) | (png[23] & 0xFF);
			if (imageW <= 0 || imageH <= 0 || imageW > 256 || imageH > 256) {
				return;
			}
			String hash = "crosshair-" + Long.toHexString(modified);
			ArcticClient.looks().registerOwn(hash, png);
			imageKey = "look:" + hash;
		} catch (Exception e) {
			ArcticClient.platform().log(false, "crosshair picture: " + e);
		}
	}

	/** One pass; {@code grow} widens every shape (for the outline). */
	private static void draw(Gfx g, CrosshairConfig c, int cx, int cy, int grow, int color) {
		draw(g, c, c.style, cx, cy, grow, color);
	}

	private static void draw(Gfx g, CrosshairConfig c, String style, int cx, int cy, int grow, int color) {
		int t = Math.max(1, c.thickness);
		int lo = -(t / 2);
		int hi = lo + t;
		if ("cross".equals(style) || "cross-dot".equals(style)) {
			// Arms start `gap` past the center square [lo, hi) on each side, so
			// all four are the same distance from the middle.
			int in = c.gap;
			int out = c.gap + c.size;
			// Right, left, down, up arms.
			rect(g, cx + hi + in - grow, cy + lo - grow, cx + hi + out + grow, cy + hi + grow, color);
			rect(g, cx + lo - out - grow, cy + lo - grow, cx + lo - in + grow, cy + hi + grow, color);
			rect(g, cx + lo - grow, cy + hi + in - grow, cx + hi + grow, cy + hi + out + grow, color);
			rect(g, cx + lo - grow, cy + lo - out - grow, cx + hi + grow, cy + lo - in + grow, color);
		}
		if ("dot".equals(style) || "cross-dot".equals(style)) {
			rect(g, cx + lo - grow, cy + lo - grow, cx + hi + grow, cy + hi + grow, color);
		}
		if ("circle".equals(style)) {
			ring(g, cx, cy, c.gap + c.size + grow, Math.max(0, c.gap + c.size - t - grow), color);
		}
	}

	private static void rect(Gfx g, int x0, int y0, int x1, int y1, int color) {
		if (x1 > x0 && y1 > y0) {
			g.fill(x0, y0, x1, y1, color);
		}
	}

	/** A pixel ring between two radii. */
	private static void ring(Gfx g, int cx, int cy, int outer, int inner, int color) {
		for (int dy = -outer; dy < outer; dy++) {
			double yc = dy + 0.5;
			int o = (int) Math.round(Math.sqrt(Math.max(0, outer * outer - yc * yc)));
			int i = inner * inner > yc * yc ? (int) Math.round(Math.sqrt(inner * inner - yc * yc)) : 0;
			if (i == 0) {
				rect(g, cx - o, cy + dy, cx + o, cy + dy + 1, color);
			} else {
				rect(g, cx - o, cy + dy, cx - i, cy + dy + 1, color);
				rect(g, cx + i, cy + dy, cx + o, cy + dy + 1, color);
			}
		}
	}
}
