package com.arcticlauncher.legacy;

import com.mojang.blaze3d.platform.GlStateManager;
import java.awt.Font;
import java.awt.Graphics2D;
import java.awt.Rectangle;
import java.awt.RenderingHints;
import java.awt.font.FontRenderContext;
import java.awt.font.GlyphVector;
import java.awt.image.BufferedImage;
import java.nio.ByteBuffer;
import net.minecraft.client.texture.NativeImageBackedTexture;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL11;

/**
 * Smooth (anti-aliased) Inter letters rasterized at one larger pixel scale, for text the game draws scaled up
 * (titles at 4x, a scaled scoreboard). Letters are rasterized the first time they are drawn at this size and kept
 * in one texture page; nothing is rasterized again until the GUI scale or the font settings change.
 *
 * <p>Only the look comes from here: the pen still moves by the GUI-size advance (see {@link LegacySmoothFont}),
 * so wrapping, centring and click hit-testing measure exactly what they did.
 */
final class ScaledGlyphs {
	/** Largest pixel scale a bucket is rasterized at: em 8.5 x 16 = 136 px. Bigger text is magnified from this. */
	static final int MAX_PIXEL_SCALE = 16;
	private static final int PAD = 1;

	/** One letter in the page; positions in GUI pixels, relative to the pen and the baseline. */
	static final class Glyph {
		final float left;
		final float top;
		final float width;
		final float height;
		final float u0;
		final float v0;
		final float u1;
		final float v1;

		Glyph(float left, float top, float width, float height, float u0, float v0, float u1, float v1) {
			this.left = left;
			this.top = top;
			this.width = width;
			this.height = height;
			this.u0 = u0;
			this.v0 = v0;
			this.u1 = u1;
			this.v1 = v1;
		}
	}

	private final Font font;
	private final int pixels;
	private final int page;
	private final FontRenderContext frc = new FontRenderContext(null, true, true);
	private final Glyph[] glyphs = new Glyph[0x2200];
	/** Characters that have no shape (or don't fit): looked up once, then drawn from the GUI-size atlas. */
	private final boolean[] skipped = new boolean[0x2200];
	private int texture;
	private NativeImageBackedTexture ownedTexture;
	private int shelfX;
	private int shelfY;
	private int shelfHeight;

	/** The pixel scale for a multiple of the GUI scale, capped so a page holds a useful number of letters. */
	static int pixelScale(int guiScale, int multiple) {
		return Math.min(MAX_PIXEL_SCALE, Math.max(1, guiScale) * multiple);
	}

	ScaledGlyphs(Font inter, float size, int pixels) {
		this.font = inter.deriveFont(size * pixels);
		this.pixels = pixels;
		// Room for about a hundred letters at the largest size; small buckets get small pages.
		this.page = pixels <= 4 ? 512 : pixels <= 8 ? 1024 : 2048;
	}

	int textureId() {
		return texture;
	}

	/** The letter at this bucket's size, rasterized now if this is its first use; null to fall back. */
	Glyph glyph(char c) {
		if (c >= glyphs.length) {
			return null;
		}
		Glyph g = glyphs[c];
		if (g != null || skipped[c]) {
			return g;
		}
		try {
			g = rasterize(c);
		} catch (RuntimeException | LinkageError e) {
			g = null;
		}
		if (g == null) {
			skipped[c] = true;
		}
		glyphs[c] = g;
		return g;
	}

	private Glyph rasterize(char c) {
		if (!font.canDisplay(c)) {
			return null;
		}
		GlyphVector gv = font.createGlyphVector(frc, String.valueOf(c));
		Rectangle box = gv.getPixelBounds(frc, 0, 0);
		if (box.width <= 0 || box.height <= 0) {
			return null;
		}
		int w = box.width + PAD * 2;
		int h = box.height + PAD * 2;
		if (w > page || h > page) {
			return null;
		}
		if (!place(w, h)) {
			return null;
		}
		int x = shelfX;
		int y = shelfY;
		shelfX += w;

		BufferedImage image = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		Graphics2D g = image.createGraphics();
		g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
		g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, RenderingHints.VALUE_FRACTIONALMETRICS_ON);
		g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
		g.setFont(font);
		g.setColor(java.awt.Color.WHITE);
		g.drawString(String.valueOf(c), PAD - box.x, PAD - box.y);
		g.dispose();
		// RGBA as in the base atlas, including on the world's fixed-function/lightmap rendering path.
		ByteBuffer texels = BufferUtils.createByteBuffer(w * h * 4);
		int[] argb = image.getRGB(0, 0, w, h, null, 0, w);
		for (int pixel : argb) {
			texels.put((byte) 0xFF).put((byte) 0xFF).put((byte) 0xFF).put((byte) (pixel >>> 24));
		}
		texels.flip();
		GlStateManager.bindTexture(texture);
		GL11.glPixelStorei(GL11.GL_UNPACK_ROW_LENGTH, 0);
		GL11.glPixelStorei(GL11.GL_UNPACK_SKIP_PIXELS, 0);
		GL11.glPixelStorei(GL11.GL_UNPACK_SKIP_ROWS, 0);
		GL11.glTexSubImage2D(GL11.GL_TEXTURE_2D, 0, x, y, w, h, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, texels);

		float s = pixels;
		return new Glyph((box.x - PAD) / s, (-box.y + PAD) / s, w / s, h / s,
				x / (float) page, y / (float) page, (x + w) / (float) page, (y + h) / (float) page);
	}

	/** Finds room for a w x h letter (shelf packing), making the page on first use. */
	private boolean place(int w, int h) {
		if (texture == 0) {
			create();
		}
		if (shelfX + w > page) {
			shelfX = 0;
			shelfY += shelfHeight;
			shelfHeight = 0;
		}
		if (shelfY + h > page) {
			return false;
		}
		shelfHeight = Math.max(shelfHeight, h);
		return true;
	}

	private void create() {
		// Use the same initialization and ownership as the base font, including texture level limits.
		ownedTexture = new NativeImageBackedTexture(
				new BufferedImage(page, page, BufferedImage.TYPE_INT_ARGB));
		texture = ownedTexture.getGlId();
		GlStateManager.bindTexture(texture);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_LINEAR);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_LINEAR);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_S, GL11.GL_CLAMP);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_T, GL11.GL_CLAMP);

		ArcticLegacy.LOG.info("smooth font: letters at pixel scale {} ({}x{})", pixels, page, page);
	}

	void close() {
		if (texture != 0) {
			ownedTexture.clearGlId();
			ownedTexture = null;
			texture = 0;
		}
	}
}
