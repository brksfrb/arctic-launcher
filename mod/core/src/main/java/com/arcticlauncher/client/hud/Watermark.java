package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

import com.arcticlauncher.client.Lang;
/** A small, quiet Arctic snowflake in a corner of the screen (for screenshots and streams); the version after it if asked. */
public final class Watermark extends HudWidget {
	public static final String[] CORNERS = {"mods.watermark.bottom_left", "mods.watermark.bottom_right", "mods.watermark.top_left", "mods.watermark.top_right"};
	public static final String[] OPACITIES = {"mods.watermark.faint", "mods.watermark.medium", "mods.watermark.strong"};
	public static final int[] SIZES = {12, 16, 20, 24, 32};
	private static final int MARGIN = 6;
	private static final int GAP = 4;

	public Watermark() {
		super("watermark", Lang.t("hud.arctic_watermark"), Lang.t("hud.a_small_arctic_snowflake_in_a_corner_for"), false, Column.LEFT);
	}

	private static ClientConfig config() {
		return ArcticClient.config();
	}

	private static int size() {
		ClientConfig c = config();
		int i = c == null ? 1 : Math.max(0, Math.min(SIZES.length - 1, c.watermarkSize));
		return SIZES[i];
	}

	/** The text after the snowflake: ARCTIC and/or the launcher's version; empty when neither is on. */
	private static String label() {
		ClientConfig c = config();
		if (c == null) {
			return "";
		}
		String name = c.watermarkName ? Lang.t("hud.arctic") : "";
		String version = c.watermarkVersion ? version() : "";
		return name.isEmpty() || version.isEmpty() ? name + version : name + " " + version;
	}

	/** The text grows with the snowflake (the default size, 16, draws it at the game's own size). */
	private static float textScale() {
		return size() / 16f;
	}

	@Override
	public Object content(boolean preview) {
		ClientConfig c = config();
		return c == null ? null : c.watermarkCorner + "/" + c.watermarkSize + "/" + c.watermarkOpacity + "/" + label() + "/" + c.watermarkColor;
	}

	@Override
	public int width(Gfx g) {
		String label = label();
		return size() + (label.isEmpty() ? 0 : GAP + Math.round(g.textWidth(label) * textScale()));
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
		String label = label();
		if (!label.isEmpty()) {
			float scale = textScale();
			int color = com.arcticlauncher.client.gfx.Draw.alpha(c.watermarkColor | 0xFF000000, 0.35f + 0.25f * level);
			g.push();
			g.translate(size + GAP, (size - 8 * scale) / 2f);
			g.scale(scale);
			g.text(label, 0, 0, color, false);
			g.pop();
		}
	}
}
