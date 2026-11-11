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
 * Characters Inter lacks keep Minecraft's own font. Text the game draws scaled up (titles, a scaled
 * scoreboard) takes its letters from {@link ScaledGlyphs} size buckets instead, rasterized near their size
 * on screen; metrics (advances, line height) always come from the GUI-size atlas.
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
		/** The character (to find the same letter in a size bucket, see ScaledGlyphs). */
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
	/** Inter as loaded from the jar (sized per use with deriveFont; reading the file once is enough). */
	private static Font interFont;
	/**
	 * Text drawn larger than this many times the GUI's size takes its letters from a size bucket, rasterized near
	 * the size they land on screen, instead of magnifying the GUI-size atlas. Up to this, magnifying is not visible.
	 */
	private static final float UPSCALE_AT = 1.25f;
	/** Bucket multiples of the GUI scale: x2, x4, x8 (each made the first time text that big is drawn). */
	private static final int[] BUCKET_MULTIPLES = {2, 4, 8};
	private static final ScaledGlyphs[] buckets = new ScaledGlyphs[BUCKET_MULTIPLES.length];
	private static final java.nio.FloatBuffer MODEL = org.lwjgl.BufferUtils.createFloatBuffer(16);
	private static final java.nio.FloatBuffer PROJECTION = org.lwjgl.BufferUtils.createFloatBuffer(16);
	private static final java.nio.IntBuffer VIEWPORT = org.lwjgl.BufferUtils.createIntBuffer(16);
	private static final float[] CLIP = new float[16];
	private static float drawScale;
	/** Set only by the explicit nametagfont self-test for a matching old-atlas screenshot. */
	static boolean proofBaseline;

	/**
	 * Read the actual transform once per draw, shared by its shadow and foreground layers. Text rendering
	 * changes colour, blending and textures, but never these matrices or the viewport. Keep the reads here
	 * (rather than a cross-draw cache) so direct GL transforms from other mods are still observed.
	 */
	public static void beginDraw() {
		// The GUI-size atlas establishes the divisor, including the first draw after a GUI-scale change.
		ready();
		GL11.glGetFloat(GL11.GL_MODELVIEW_MATRIX, MODEL);
		GL11.glGetFloat(GL11.GL_PROJECTION_MATRIX, PROJECTION);
		GL11.glGetInteger(GL11.GL_VIEWPORT, VIEWPORT);
		com.arcticlauncher.client.feature.TextPixelScale.multiply(PROJECTION, MODEL, CLIP);
	}

	/** Each layer keeps its own anchor for the perspective divide, without another driver read. */
	public static void preparing(float x, float y) {
		drawScale = com.arcticlauncher.client.feature.TextPixelScale.projected(CLIP, x, y,
				VIEWPORT.get(2), VIEWPORT.get(3)) / Math.max(1, scale);
	}

	/**
	 * The size bucket for text drawn {@code times} larger than the GUI scale (null when the GUI-size atlas is
	 * already right): the smallest bucket at least that large, so its letters are only ever shrunk on screen.
	 */
	private static ScaledGlyphs bucketFor(float times) {
		if (!(times > UPSCALE_AT)) {
			return null;
		}
		int slot = BUCKET_MULTIPLES.length - 1;
		for (int i = 0; i < BUCKET_MULTIPLES.length; i++) {
			if (BUCKET_MULTIPLES[i] >= times) {
				slot = i;
				break;
			}
		}
		int pixels = ScaledGlyphs.pixelScale(scale, BUCKET_MULTIPLES[slot]);
		if (pixels <= scale) {
			return null;
		}
		ScaledGlyphs bucket = buckets[slot];
		if (bucket == null) {
			try {
				bucket = new ScaledGlyphs(font(), SIZE, pixels);
			} catch (Exception e) {
				ArcticLegacy.LOG.warn("smooth font (x{} letters): {}", BUCKET_MULTIPLES[slot], e.toString());
				return null;
			}
			buckets[slot] = bucket;
		}
		return bucket;
	}

	private static void dropBuckets() {
		for (int i = 0; i < buckets.length; i++) {
			if (buckets[i] != null) {
				buckets[i].close();
				buckets[i] = null;
			}
		}
	}

	/** Inter at 1 pt, read from the jar the first time. */
	private static Font font() throws Exception {
		if (interFont == null) {
			try (InputStream in = LegacySmoothFont.class.getResourceAsStream(FILE)) {
				if (in == null) {
					throw new IllegalStateException(FILE + " is missing from the mod");
				}
				interFont = Font.createFont(Font.TRUETYPE_FONT, in);
			}
		}
		return interFont;
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
		// Most installs have no Patcher. This is queried by glyph draws and width lookups too;
		// once absence is known, they need neither reflection nor a clock read.
		if (patcherLooked && patcherOptimized == null) {
			return;
		}
		long now = System.nanoTime();
		if (now - patcherCheckedAt < 2_000_000_000L && patcherLooked) {
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

	/**
	 * Whole GUI pixels the pen moves for this character. Drawing, wrapping and hit-testing all use
	 * this. The 1.8 renderer keeps only the whole pixels of whatever a glyph returns, so a raw
	 * 2.5 would be drawn as 2 while a width sum still counted 2.5.
	 */
	public static int step(char c) {
		float advance = advance(c);
		return advance < 0 ? -1 : com.arcticlauncher.client.feature.TextHit.step(advance);
	}

	/**
	 * Fancy font metrics are the ones on screen. Set {@code -Darctic.chatHitOld=1} to draw and hit
	 * the old way (the screenshot that shows the miss). Off, or with Fancy off, this is false.
	 */
	public static boolean aligned() {
		return on() && !"1".equals(System.getProperty("arctic.chatHitOld"));
	}

	/** Chat and book line box, from Inter's ascent, descent and leading. At least the vanilla 9. */
	private static int linePixels = 9;

	public static int lineHeight() {
		if (on()) {
			ready();
		}
		return Math.max(9, linePixels);
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
		ScaledGlyphs bucket = bucketFor(proofBaseline && Math.abs(PROJECTION.get(15)) < 1e-6f ? 0 : drawScale);
		ScaledGlyphs.Glyph b = bucket == null ? null : bucket.glyph(g.ch);
		if (b != null) {
			// Drawn from letters rasterized at (or just above) their size on screen, shrunk by the linear filter.
			// The quad comes from the big letter's own box; the pen moves by the GUI-size advance, so the layout the
			// game measured (wrapping, centring, click hit-testing) holds.
			float bx0 = penX + b.left;
			float by0 = penY + BASELINE - b.top;
			float bslant = italic ? 1f : 0f;
			GlStateManager.bindTexture(bucket.textureId());
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
			return g.advance;
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
		dropBuckets();
	}

	private static Atlas make(int s, boolean aa) throws Exception {
		Font font = font().deriveFont(SIZE * s);
		FontRenderContext frc = new FontRenderContext(null, true, true);
		java.awt.font.LineMetrics metrics = font.getLineMetrics("Agjpq[]|", frc);
		linePixels = Math.max(9, Math.round(metrics.getHeight() / s));
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
				// ...and never narrower than the letter's own shape (checked by drawing a sentence both ways), so a
				// letter never runs into the next one.
				float shape = (float) Math.ceil(gv.getOutline().getBounds2D().getMaxX());
				advances.add(aa ? advance : Math.max(Math.max(1f, Math.round(advance)), shape));
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
		// Smooth letters are drawn with a smooth filter; pixel-sharp ones with the
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
