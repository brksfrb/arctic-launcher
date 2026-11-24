package com.arcticlauncher.client.gfx;

import java.awt.Color;
import java.awt.Font;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.font.FontRenderContext;
import java.awt.image.BufferedImage;
import java.io.InputStream;

/** Inter at GUI scale, in a bounded atlas that can be bound without changing the game's font. */
public final class MenuFont {
	private static final int MAX_SCALE = 4;
	private static final int COLUMNS = 32;
	private static final int CELL = 13;
	private static final int[] RANGES = {32, 383, 0x400, 0x52f, 0x2000, 0x203f};
	private static final MenuFont[] SCALES = new MenuFont[MAX_SCALE];
	private static boolean unavailable;

	private final Glyph[] glyphs = new Glyph[0x2040];
	private final BufferedImage image;
	private final int scale;

	private MenuFont(Font font, int scale) {
		this.scale = scale;
		int cell = CELL * scale;
		int count = 0;
		for (int i = 0; i < RANGES.length; i += 2) count += RANGES[i + 1] - RANGES[i] + 1;
		image = new BufferedImage(COLUMNS * cell, ((count + COLUMNS - 1) / COLUMNS) * cell, BufferedImage.TYPE_INT_ARGB);
		Graphics2D graphics = image.createGraphics();
		try {
			graphics.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
			graphics.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, RenderingHints.VALUE_FRACTIONALMETRICS_ON);
			graphics.setColor(Color.WHITE);
			Font sized = font.deriveFont(8.5f * scale);
			FontRenderContext context = graphics.getFontRenderContext();
			char[] input = new char[1];
			int index = 0;
			for (int i = 0; i < RANGES.length; i += 2) {
				for (int c = RANGES[i]; c <= RANGES[i + 1]; c++, index++) {
					input[0] = (char) c;
					if (!sized.canDisplay(input[0])) continue;
					java.awt.font.GlyphVector vector = sized.createGlyphVector(context, input);
					int x = index % COLUMNS * cell + scale;
					int y = index / COLUMNS * cell + 8 * scale;
					graphics.drawGlyphVector(vector, x, y);
					float width = vector.getGlyphMetrics(0).getAdvanceX() / scale;
					glyphs[c] = new Glyph(x - scale, y - 8 * scale, Math.min(CELL, (int) Math.ceil(width) + 3), c == ' ' ? 2.5f : width);
				}
			}
		} finally {
			graphics.dispose();
		}
	}

	public static MenuFont at(float guiScale) {
		if (unavailable) return null;
		int scale = Math.min(MAX_SCALE, Math.max(1, Math.round(guiScale)));
		MenuFont font = SCALES[scale - 1];
		if (font != null) return font;
		try (InputStream in = MenuFont.class.getResourceAsStream("/resourcepacks/smooth_font/assets/arctic/font/inter.ttf");
			 InputStream legacy = in == null ? MenuFont.class.getResourceAsStream("/smooth_font/assets/arctic/font/inter.ttf") : null) {
			InputStream source = in == null ? legacy : in;
			if (source != null) {
				font = new MenuFont(Font.createFont(Font.TRUETYPE_FONT, source), scale);
				SCALES[scale - 1] = font;
				return font;
			}
		} catch (Exception | LinkageError ignored) {
			// A stripped Java runtime or unreadable font falls back to the game's font.
		}
		unavailable = true;
		return null;
	}

	public BufferedImage image() { return image; }
	public int scale() { return scale; }

	public int width(String text) {
		float width = 0;
		for (int i = 0; i < text.length(); i++) {
			char c = text.charAt(i);
			if (format(text, i)) {
				i++;
			} else {
				Glyph glyph = c < glyphs.length ? glyphs[c] : null;
				if (glyph == null) return -1;
				width += glyph.advance;
			}
		}
		return Math.round(width);
	}

	private static boolean format(String text, int at) {
		return text.charAt(at) == '§' && at + 1 < text.length() && "lr".indexOf(text.charAt(at + 1)) >= 0;
	}

	public interface DrawGlyph {
		void glyph(int x, int y, int w, int h, int u, int v, int scale, int imageW, int imageH, int color);
	}

	public void draw(DrawGlyph target, String text, int x, int y, int color, boolean shadow) {
		if (shadow) {
			int shade = Math.round((color >>> 24) * .45f) << 24;
			drawLine(target, text, x + 1, y + 1, shade);
		}
		drawLine(target, text, x, y, color);
	}

	private void drawLine(DrawGlyph target, String text, int x, int y, int color) {
		float cursor = x;
		for (int i = 0; i < text.length(); i++) {
			char c = text.charAt(i);
			if (format(text, i)) {
				i++;
				continue;
			}
			Glyph glyph = glyphs[c];
			if (c != ' ') {
				target.glyph(Math.round(cursor) - 1, y - 1, glyph.width, CELL, glyph.u, glyph.v, scale,
						image.getWidth(), image.getHeight(), color);
			}
			cursor += glyph.advance;
		}
	}

	private static final class Glyph {
		final int u, v, width;
		final float advance;

		Glyph(int u, int v, int width, float advance) {
			this.u = u;
			this.v = v;
			this.width = width;
			this.advance = advance;
		}
	}
}
