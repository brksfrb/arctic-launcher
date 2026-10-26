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
		/** The character (to find the same letter in the big atlas). */
		char ch;
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
	/** Smooth letters at several times the size, for titles and other big text in pixel-sharp mode (made when first needed). */
	private static Atlas bigAtlas;
	private static int bigFor;
	/** Text bigger than this many times the GUI's size is drawn from the big atlas. */
	private static final float BIG_AT = 2.5f;
	/** The model view's uniform scale as the game has set it up (tracked from GlStateManager.scale/push/pop). */
	private static float modelScale = 1f;
	private static final float[] SCALE_STACK = new float[64];
	private static int scaleDepth;

	public static void matrixPush() {
		if (scaleDepth < SCALE_STACK.length) {
			SCALE_STACK[scaleDepth] = modelScale;
		}
		scaleDepth++;
	}

	public static void matrixPop() {
		if (scaleDepth > 0) {
			scaleDepth--;
			if (scaleDepth < SCALE_STACK.length) {
				modelScale = SCALE_STACK[scaleDepth];
			}
		}
	}

	public static void matrixScale(float factor) {
		modelScale *= factor;
	}

	public static void matrixIdentity() {
		modelScale = 1f;
		scaleDepth = 0;
	}

	/** The big atlas, made when the first big text needs it. */
	private static Atlas big() {
		if (bigAtlas == null || bigFor != scale) {
			try {
				bigAtlas = make(Math.min(8, Math.max(1, scale) * 4), true);
				bigFor = scale;
			} catch (Exception e) {
				bigAtlas = null;
				bigFor = scale;
				ArcticLegacy.LOG.warn("smooth font (big letters): {}", e.toString());
			}
		}
		return bigAtlas;
	}

	private LegacySmoothFont() {}

	/** The letters were last drawn with (or without) anti-aliasing. */
	private static boolean builtAntialias;

	private static boolean antialias() {
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		return config != null && config.fontAntialias;
	}

	/** Anti-aliasing was switched: the letters are drawn again at the next use. */
	public static void rebuild() {
		scale = 0;
	}

	public static boolean on() {
		// Forge's loading screen draws text before the client has started: no config yet.
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		boolean on = config != null && config.fancy && !broken;
		if (on) {
			yieldPatcher();
		}
		return on;
	}

	private static long patcherCheckedAt;
	private static java.lang.reflect.Field patcherOptimized;
	private static boolean patcherLooked;

	/**
	 * Patcher's "optimized font renderer" draws every string itself from the game's bitmap font, so Inter never
	 * shows. While Fancy is on it is switched off in memory (Patcher's own settings file is not touched).
	 */
	private static void yieldPatcher() {
		long now = System.nanoTime();
		if (patcherLooked && patcherOptimized == null || now - patcherCheckedAt < 2_000_000_000L && patcherLooked) {
			return;
		}
		patcherCheckedAt = now;
		try {
			if (!patcherLooked) {
				patcherLooked = true;
				patcherOptimized = Class.forName("club.sk1er.patcher.config.PatcherConfig").getField("optimizedFontRenderer");
			}
			if (patcherOptimized != null && patcherOptimized.getBoolean(null)) {
				patcherOptimized.setBoolean(null, false);
			}
		} catch (ClassNotFoundException | NoSuchFieldException e) {
			patcherOptimized = null;
		} catch (Exception | LinkageError e) {
			patcherOptimized = null;
		}
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

	/** How far a bold character's second copy sits (GUI pixels): thicker, not doubled. */
	public static final float BOLD_THICKEN = 0.35f;

	/** Shadow offset: about half a GUI pixel, in whole screen pixels (a whole one looks doubled). */
	public static float shadowOffset() {
		int s = Math.max(1, scale);
		return Math.max(1, Math.round(s * 0.5f)) / (float) s;
	}

	/**
	 * Draw a glyph at the pen (top of the line) in the current colour; returns how far the pen moves.
	 * Blending is on for the whole string (SmoothTextRendererMixin), not asked for per letter: asking
	 * the driver for its state stalls it, and the HUD draws hundreds of letters a frame.
	 */
	public static float draw(Glyph g, float penX, float penY, boolean italic) {
		if (modelScale >= BIG_AT && !antialias()) {
			Atlas atlas = big();
			Glyph b = atlas != null && g.ch < atlas.glyphs.length ? atlas.glyphs[g.ch] : null;
			if (b != null) {
				float bx0 = penX + b.left;
				float by0 = penY + BASELINE - b.top;
				float bslant = italic ? 1f : 0f;
				GlStateManager.bindTexture(atlas.texture.getGlId());
				GL11.glBegin(GL11.GL_QUADS);
				GL11.glTexCoord2f(b.u0, b.v0);
				GL11.glVertex3f(bx0 + bslant, by0, 0f);
				GL11.glTexCoord2f(b.u0, b.v1);
				GL11.glVertex3f(bx0 - bslant, by0 + b.height, 0f);
				GL11.glTexCoord2f(b.u1, b.v1);
				GL11.glVertex3f(bx0 + b.width - bslant, by0 + b.height, 0f);
				GL11.glTexCoord2f(b.u1, b.v0);
				GL11.glVertex3f(bx0 + b.width + bslant, by0, 0f);
				GL11.glEnd();
				// The pen moves as the sharp letters do, so the layout the game measured holds.
				return g.advance;
			}
		}
		float s = scale;
		// Whole screen pixels, so texels and pixels line up.
		float x0 = Math.round((penX + g.left) * s) / s;
		float y0 = Math.round((penY + BASELINE - g.top) * s) / s;
		float x1 = x0 + g.width;
		float y1 = y0 + g.height;
		float slant = italic ? 1f : 0f;
		GlStateManager.bindTexture(texture.getGlId());
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
		if (checkedScale != scale || builtAntialias != antialias()) {
			try {
				builtAntialias = antialias();
				build(checkedScale);
			} catch (Exception e) {
				broken = true;
				ArcticLegacy.LOG.warn("smooth font: {}", e.toString());
				return false;
			}
		}
		return texture != null;
	}

	/** The letters and the texture holding them, for one pixel scale. */
	private static final class Atlas {
		final Glyph[] glyphs;
		final NativeImageBackedTexture texture;

		Atlas(Glyph[] glyphs, NativeImageBackedTexture texture) {
			this.glyphs = glyphs;
			this.texture = texture;
		}
	}

	private static void build(int s) throws Exception {
		Atlas atlas = make(s, antialias());
		if (texture != null) {
			texture.clearGlId();
		}
		texture = atlas.texture;
		glyphs = atlas.glyphs;
		scale = s;
		if (bigAtlas != null) {
			bigAtlas.texture.clearGlId();
			bigAtlas = null;
		}
	}

	private static Atlas make(int s, boolean aa) throws Exception {
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
				float advance = (float) gv.getGlyphMetrics(0).getAdvanceX();
				// Pixel-sharp text steps whole pixels from letter to letter: a letter drawn between two pixels
				// is smeared across both.
				advances.add(aa ? advance : Math.max(1f, Math.round(advance)));
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
		g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, aa
				? RenderingHints.VALUE_TEXT_ANTIALIAS_ON : RenderingHints.VALUE_TEXT_ANTIALIAS_OFF);
		g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, aa
				? RenderingHints.VALUE_FRACTIONALMETRICS_ON : RenderingHints.VALUE_FRACTIONALMETRICS_OFF);
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
			glyph.ch = c;
			made[c] = glyph;
		}
		g.dispose();
		NativeImageBackedTexture made2 = new NativeImageBackedTexture(image);
		// Smooth letters are drawn with a smooth filter (titles are 4x); pixel-sharp ones with the
		// nearest-pixel filter, so a letter on whole pixels stays whole.
		GlStateManager.bindTexture(made2.getGlId());
		int filter = aa ? GL11.GL_LINEAR : GL11.GL_NEAREST;
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, filter);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, filter);
		ArcticLegacy.LOG.info("smooth font: {} characters at GUI scale {} ({}x{}, {})", chars.size(), s, width, height,
				aa ? "smooth" : "sharp");
		return new Atlas(made, made2);
	}
}
