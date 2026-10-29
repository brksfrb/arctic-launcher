package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** A small, quiet Arctic snowflake in a corner of the screen (for screenshots and streams); the version after it if asked. */
public final class Watermark extends HudWidget {
	public static final String[] CORNERS = {"Bottom left", "Bottom right", "Top left", "Top right"};
	public static final String[] OPACITIES = {"Faint", "Medium", "Strong"};
	public static final int[] SIZES = {12, 16, 20, 24, 32};
	private static final int MARGIN = 6;
	private static final int GAP = 4;

	public Watermark() {
		super("watermark", "Arctic watermark", "A small Arctic snowflake in a corner, for screenshots and streams", false, Column.LEFT);
	}

	private static ClientConfig config() {
		return ArcticClient.config();
	}

	private static int size() {
		ClientConfig c = config();
		int i = c == null ? 1 : Math.max(0, Math.min(SIZES.length - 1, c.watermarkSize));
		return SIZES[i];
	}

	private static boolean withVersion() {
		ClientConfig c = config();
		return c != null && c.watermarkVersion;
	}

	@Override
	public Object content(boolean preview) {
		ClientConfig c = config();
		return c == null ? null : c.watermarkCorner + "/" + c.watermarkSize + "/" + c.watermarkOpacity + "/" + c.watermarkVersion;
	}

	@Override
	public int width(Gfx g) {
		return size() + (withVersion() ? GAP + g.textWidth(version()) : 0);
	}

	@Override
	public int height() {
		return size();
	}

	private static String version() {
		String version = System.getProperty("arctic.launcher");
		return version == null || version.isEmpty() ? "Arctic" : version;
	}

	/** In its corner until the player moves it. */
	@Override
	public int[] home(Gfx g) {
		ClientConfig c = config();
		int corner = c == null ? 0 : c.watermarkCorner;
		int w = width(g);
		int h = height();
		int x = corner == 1 || corner == 3 ? g.width() - w - MARGIN : MARGIN;
		int y = corner == 0 || corner == 1 ? g.height() - h - MARGIN : MARGIN;
		return new int[] {x, y, w, h};
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		ClientConfig c = config();
		int level = c == null ? 1 : Math.max(0, Math.min(2, c.watermarkOpacity));
		int size = size();
		g.texture("asset:watermark-" + (level + 1), 0, 0, size, size, 0, 0, 64, 64, 64, 64);
		if (withVersion()) {
			g.text(version(), size + GAP, (size - 8) / 2, com.arcticlauncher.client.gfx.Draw.alpha(0xFFFFFFFF, 0.35f + 0.25f * level), false);
		}
	}
}
