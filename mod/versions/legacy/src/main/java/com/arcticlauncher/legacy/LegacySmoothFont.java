package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.GlStateManager;
import java.awt.Font;
import java.awt.Graphics2D;
import java.awt.Rectangle;
import java.awt.RenderingHints;
import java.awt.font.FontRenderContext;
import java.awt.font.GlyphVector;
import java.awt.image.BufferedImage;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.client.util.Window;
import org.lwjgl.opengl.GL11;

/**
 * The smooth font (Inter) on 1.8.9 - 1.12.2, which have no TrueType fonts: with Fancy on, Inter's
 * glyphs are drawn into one texture at the GUI scale (so each texel lands on one screen pixel),
 * and the text renderer draws and measures characters from it (see SmoothTextRendererMixin).
 * Characters Inter lacks keep Minecraft's own font.
 */
public final class LegacySmoothFont {
	private static final String FILE = "/smooth_font/assets/arctic/font/inter.ttf";
	/** Em size in GUI pixels, as the newer versions' smooth font. */
	private static final float SIZE = 8.5f;
	/** Where the baseline sits below the top of a line, in GUI pixels (Minecraft's font: 7). */
	private static final float BASELINE = 7f;
	/** Inter's space is narrow for Minecraft text; the newer versions use this too. */
	private static final float SPACE = 2.5f;
	private static final int PAD = 1;
	private static final long SCALE_RECHECK_MS = 500;
	/** Unicode blocks drawn: Latin, Greek, Cyrillic and common punctuation. */
	private static final int[][] RANGES = {
			{0x21, 0x7E}, {0xA1, 0x24F}, {0x370, 0x3FF}, {0x400, 0x4FF},
			{0x2010, 0x2027}, {0x2030, 0x203A}, {0x20AC, 0x20AC}, {0x2122, 0x2122},
	};

	/** One character in the texture; positions in GUI pixels, relative to the pen and the baseline. */
	public static final class Glyph {
		final float advance;
		final float left;
		final float top;
		final float width;
		final float height;
		float u0;
		float v0;
		float u1;
		float v1;

		Glyph(float advance, float left, float top, float width, float height) {
			this.advance = advance;
			this.left = left;
			this.top = top;
			this.width = width;
			this.height = height;
		}
	}

	private static Glyph[] glyphs = new Glyph[0];
	private static NativeImageBackedTexture texture;
	private static int scale;
	private static int checkedScale;
	private static long checkedAt;
	private static boolean broken;
	/** The GL colour, read back when drawing without blending (16 floats: glGetFloat wants room for any query). */
	private static final java.nio.FloatBuffer COLOR = org.lwjgl.BufferUtils.createFloatBuffer(16);

	private LegacySmoothFont() {}

	public static boolean on() {
		return ArcticClient.config().fancy && !broken;
	}

	/** The glyph for a character, or null when Minecraft's own font should draw it. */
	public static Glyph glyph(char c) {
		if (!ready()) {
			return null;
		}
		return c < glyphs.length ? glyphs[c] : null;
	}

	public static float advance(char c) {
		if (c == ' ') {
			return SPACE;
		}
		Glyph g = glyph(c);
		return g == null ? -1 : g.advance;
	}

	/** Shadow offset: about half a GUI pixel, in whole screen pixels (a whole one looks doubled). */
	public static float shadowOffset() {
		int s = Math.max(1, scale);
		return Math.max(1, Math.round(s * 0.5f)) / (float) s;
	}

	/**
	 * Draw a glyph at the pen (top of the line) in the text's colour; returns how far the pen moves.
	 * Its edges need blending; where the game draws text without it (the scoreboard, most HUD
	 * text), the colour's alpha is left out as Minecraft's own font ignores it there, or that text
	 * would come out faded.
	 */
	public static float draw(Glyph g, float penX, float penY, boolean italic) {
		float s = scale;
		// Whole screen pixels, so texels and pixels line up.
		float x0 = Math.round((penX + g.left) * s) / s;
		float y0 = Math.round((penY + BASELINE - g.top) * s) / s;
		float x1 = x0 + g.width;
		float y1 = y0 + g.height;
		float slant = italic ? 1f : 0f;
		GlStateManager.bindTexture(texture.getGlId());
		boolean blend = GL11.glIsEnabled(GL11.GL_BLEND);
		if (!blend) {
			GlStateManager.enableBlend();
			GlStateManager.blendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
			// The current colour (formatting codes set it without telling the text renderer's fields).
			GL11.glGetFloat(GL11.GL_CURRENT_COLOR, COLOR);
			GL11.glColor4f(COLOR.get(0), COLOR.get(1), COLOR.get(2), 1f);
		}
		GL11.glBegin(GL11.GL_QUADS);
		GL11.glTexCoord2f(g.u0, g.v0);
		GL11.glVertex3f(x0 + slant, y0, 0f);
		GL11.glTexCoord2f(g.u0, g.v1);
		GL11.glVertex3f(x0 - slant, y1, 0f);
		GL11.glTexCoord2f(g.u1, g.v1);
		GL11.glVertex3f(x1 - slant, y1, 0f);
		GL11.glTexCoord2f(g.u1, g.v0);
		GL11.glVertex3f(x1 + slant, y0, 0f);
		GL11.glEnd();
		if (!blend) {
			GL11.glColor4f(COLOR.get(0), COLOR.get(1), COLOR.get(2), COLOR.get(3));
			GlStateManager.disableBlend();
		}
		return g.advance;
	}

	/** The texture exists for the current GUI scale (made again when the scale changes). */
	private static boolean ready() {
		if (!on()) {
			return false;
		}
		long now = System.currentTimeMillis();
		if (checkedScale == 0 || now - checkedAt > SCALE_RECHECK_MS) {
			checkedScale = Math.max(1, new Window(MinecraftClient.getInstance()).getScaleFactor());
			checkedAt = now;
		}
		if (checkedScale != scale) {
			try {
				build(checkedScale);
			} catch (Exception e) {
				broken = true;
				ArcticLegacy.LOG.warn("smooth font: {}", e.toString());
				return false;
			}
		}
		return texture != null;
	}

	private static void build(int s) throws Exception {
		Font font;
		try (InputStream in = LegacySmoothFont.class.getResourceAsStream(FILE)) {
			if (in == null) {
				throw new IllegalStateException(FILE + " is missing from the mod");
			}
			font = Font.createFont(Font.TRUETYPE_FONT, in).deriveFont(SIZE * s);
		}
		FontRenderContext frc = new FontRenderContext(null, true, true);
		List<Character> chars = new ArrayList<Character>();
		List<Rectangle> boxes = new ArrayList<Rectangle>();
		List<Float> advances = new ArrayList<Float>();
		for (int[] range : RANGES) {
			for (int c = range[0]; c <= range[1]; c++) {
				if (!font.canDisplay(c)) {
					continue;
				}
				GlyphVector gv = font.createGlyphVector(frc, String.valueOf((char) c));
				Rectangle box = gv.getPixelBounds(frc, 0, 0);
				chars.add((char) c);
				boxes.add(box);
				advances.add((float) gv.getGlyphMetrics(0).getAdvanceX());
			}
		}
		// Shelf packing into a texture as wide as needed for the scale.
		int width = s >= 4 ? 2048 : 1024;
		int[] xs = new int[chars.size()];
		int[] ys = new int[chars.size()];
		int x = 0;
		int y = 0;
		int row = 0;
		for (int i = 0; i < chars.size(); i++) {
			Rectangle b = boxes.get(i);
			int w = b.width + PAD * 2;
			int h = b.height + PAD * 2;
			if (x + w > width) {
				x = 0;
				y += row;
				row = 0;
			}
			xs[i] = x;
			ys[i] = y;
			x += w;
			row = Math.max(row, h);
		}
		int height = 1;
		while (height < y + row) {
			height <<= 1;
		}
		BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
		Graphics2D g = image.createGraphics();
		g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
		g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, RenderingHints.VALUE_FRACTIONALMETRICS_ON);
		g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
		g.setFont(font);
		g.setColor(java.awt.Color.WHITE);
		Glyph[] made = new Glyph[0x2200];
		for (int i = 0; i < chars.size(); i++) {
			Rectangle b = boxes.get(i);
			char c = chars.get(i);
			if (b.width > 0 && b.height > 0) {
				g.drawString(String.valueOf(c), xs[i] + PAD - b.x, ys[i] + PAD - b.y);
			}
			Glyph glyph = new Glyph(advances.get(i) / s, (b.x - PAD) / (float) s, (-b.y + PAD) / (float) s,
					(b.width + PAD * 2) / (float) s, (b.height + PAD * 2) / (float) s);
			glyph.u0 = xs[i] / (float) width;
			glyph.v0 = ys[i] / (float) height;
			glyph.u1 = (xs[i] + b.width + PAD * 2) / (float) width;
			glyph.v1 = (ys[i] + b.height + PAD * 2) / (float) height;
			made[c] = glyph;
		}
		g.dispose();
		if (texture != null) {
			texture.clearGlId();
		}
		texture = new NativeImageBackedTexture(image);
		glyphs = made;
		scale = s;
		ArcticLegacy.LOG.info("smooth font: {} characters at GUI scale {} ({}x{})", chars.size(), s, width, height);
	}
}
