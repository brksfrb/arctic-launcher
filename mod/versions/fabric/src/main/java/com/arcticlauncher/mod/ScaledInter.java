package com.arcticlauncher.mod;

//#if MC >= 1.21.9
import com.mojang.blaze3d.font.GlyphBitmap;
import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.TrueTypeGlyphProvider;
import com.mojang.blaze3d.font.UnbakedGlyph;
import it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.ints.IntSet;
import java.io.InputStream;
import java.nio.ByteBuffer;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.font.GlyphStitcher;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.providers.FreeTypeUtil;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.Style;
//#if MC >= 1.21.11
import net.minecraft.resources.Identifier;
//#else
import net.minecraft.resources.ResourceLocation;
//#endif
import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.util.freetype.FT_Face;
import org.lwjgl.util.freetype.FreeType;

import com.arcticlauncher.client.Lang;
/**
 * The smooth font drawn scaled up (titles at 4x, a scaled scoreboard or HUD) stays sharp: Inter is rasterized at
 * the GUI scale, so magnified text used to show its texels. The projected footprint of each GUI or world
 * text draw (see {@link TextDrawScale}) selects the bucket. Each Inter letter is drawn from a size bucket rasterized near its on-screen size (x2, x4 or x8
 * the GUI scale, capped) and shrunk with a linear filter.
 *
 * <p>Only the texture changes. A swapped letter keeps the GUI-size glyph's {@link GlyphInfo}, so its advance, bold
 * and shadow offsets, and every width, wrap and click position, are exactly as before. Buckets are made the first
 * time text that big is drawn and fill one letter at a time as letters are first used (never per frame); they are
 * dropped when the GUI scale changes or the smooth font is switched off.
 */
public final class ScaledInter {
	private static final String FILE = "/resourcepacks/smooth_font/assets/arctic/font/inter.ttf";
	/** Em size in GUI pixels, as the smooth font everywhere. */
	private static final float SIZE = 8.5f;
	/** Up to this much bigger than the GUI scale, magnifying the GUI-size letters doesn't show. */
	private static final float UPSCALE_AT = 1.25f;
	private static final int[] MULTIPLES = {2, 4, 8};
	/** Largest pixel scale a bucket is rasterized at: em 136 px, so the biggest letter fits a 256 px font texture. */
	private static final int MAX_PIXEL_SCALE = 16;
	/** Letters one bucket holds at most; further ones are drawn from the GUI-size font. */
	private static final int MAX_GLYPHS = 512;

	private static final Bucket[] buckets = new Bucket[MULTIPLES.length];
	private static int bucketsFor;
	private static final boolean[] failed = new boolean[MULTIPLES.length];
	/** A bucket's font texture is being made: it filters linearly (see FontTextureMixin). */
	private static boolean baking;

	private ScaledInter() {}

	public static boolean baking() {
		return baking;
	}

	/** The glyph to draw for {@code codepoint}: {@code base} itself, or the same letter from a size bucket. */
	public static BakedGlyph glyph(int codepoint, Style style, BakedGlyph base) {
		int gui = baseScale();
		if (gui <= 0) {
			if (bucketsFor != 0) clear();
			return base;
		}
		float times = TextDrawScale.current();
		if (!(times > UPSCALE_AT) || base == null || codepoint == ' ' || style.isObfuscated() || !usesInter(style)) {
			return base;
		}
		if (gui != bucketsFor) {
			clear();
			bucketsFor = gui;
		}
		Bucket bucket = bucket(times, gui);
		return bucket == null ? base : bucket.glyph(codepoint, base);
	}

	private static boolean usesInter(Style style) {
		if (FontDescription.DEFAULT.equals(style.getFont())) return true;
		//#if MC >= 26.1
		if (style.getFont() instanceof FontDescription.Resource resource) {
			return ArcticPacks.TITLE_FONT.equals(resource.id()) || ArcticPacks.SUBTITLE_FONT.equals(resource.id());
		}
		//#endif
		return false;
	}

	/** The GUI scale the smooth font is rasterized at now, or 0 when it is off. */
	private static int baseScale() {
		//#if MC >= 26.1
		return ArcticPacks.smoothFontCached() ? ArcticPacks.loadedOversample() : 0;
		//#else
		return SmoothFont.on() ? SmoothFont.loadedScale() : 0;
		//#endif
	}

	/** The index of the smallest multiple at least {@code times} (the largest one past x8). */
	private static int slotFor(float times) {
		for (int i = 0; i < MULTIPLES.length; i++) {
			if (MULTIPLES[i] >= times) {
				return i;
			}
		}
		return MULTIPLES.length - 1;
	}

	/**
	 * Which bucket text drawn {@code times} the GUI scale takes its letters from: 0 for the GUI-size font, else 1..3
	 * (a different bucket gives different letters). For caches that keep laid-out text and must keep one per bucket.
	 */
	public static int variant(float times) {
		int gui = baseScale();
		if (gui <= 0 || !(times > UPSCALE_AT)) {
			return 0;
		}
		int slot = slotFor(times);
		return Math.min(MAX_PIXEL_SCALE, gui * MULTIPLES[slot]) <= gui ? 0 : slot + 1;
	}

	/** The smallest bucket at least {@code times} the GUI scale (the largest one past x8). */
	private static Bucket bucket(float times, int gui) {
		int slot = slotFor(times);
		int pixels = Math.min(MAX_PIXEL_SCALE, gui * MULTIPLES[slot]);
		if (pixels <= gui || failed[slot]) {
			return null;
		}
		Bucket bucket = buckets[slot];
		if (bucket == null) {
			bucket = Bucket.open(pixels, MULTIPLES[slot]);
			if (bucket == null) {
				failed[slot] = true;
				return null;
			}
			buckets[slot] = bucket;
		}
		return bucket;
	}

	public static void clear() {
		//#if MC >= 26.1
		// Cached GUI text must stop replaying before any of its textures are closed.
		GuiRecording.fontsReloaded();
		//#endif
		for (int i = 0; i < buckets.length; i++) {
			failed[i] = false;
			if (buckets[i] != null) {
				buckets[i].close();
				buckets[i] = null;
			}
		}
		bucketsFor = 0;
	}

	/** Inter at one pixel scale: its provider, the textures its letters are stitched into, and the letters so far. */
	private static final class Bucket implements UnbakedGlyph.Stitcher {
		private final TrueTypeGlyphProvider provider;
		private final GlyphStitcher stitcher;
		private final IntSet supported;
		/** By codepoint: the swapped glyph, or {@link #NONE} when this bucket can't draw it. */
		private final Int2ObjectOpenHashMap<Object> glyphs = new Int2ObjectOpenHashMap<>();
		private static final Object NONE = new Object();

		private Bucket(TrueTypeGlyphProvider provider, GlyphStitcher stitcher) {
			this.provider = provider;
			this.stitcher = stitcher;
			this.supported = provider.getSupportedGlyphs();
		}

		static Bucket open(int pixels, int multiple) {
			ByteBuffer font = read();
			if (font == null) {
				return null;
			}
			FT_Face face = null;
			TrueTypeGlyphProvider provider = null;
			try {
				synchronized (FreeTypeUtil.LIBRARY_LOCK) {
					try (MemoryStack stack = MemoryStack.stackPush()) {
						PointerBuffer handle = stack.mallocPointer(1);
						FreeTypeUtil.assertError(FreeType.FT_New_Memory_Face(FreeTypeUtil.getLibrary(), font, 0L, handle), "Initializing font face"); // i18n-ignore: an error label
						face = FT_Face.create(handle.get());
					}
					if (FreeType.FT_Select_Charmap(face, FreeType.FT_ENCODING_UNICODE) != 0) {
						throw new IllegalStateException("no unicode charmap in Inter");
					}
				}
				// The provider owns the face and the font's memory from here.
				provider = new TrueTypeGlyphProvider(font, face, SIZE, pixels, 0f, 0f, "");
				face = null;
				font = null;
				//#if MC >= 1.21.11
				Identifier id = Identifier.fromNamespaceAndPath("arctic", "smooth_font_x" + multiple);
				//#else
				ResourceLocation id = ResourceLocation.fromNamespaceAndPath("arctic", "smooth_font_x" + multiple);
				//#endif
				Bucket bucket = new Bucket(provider, new GlyphStitcher(Minecraft.getInstance().getTextureManager(), id));
				ArcticMod.LOG.info("smooth font: x{} letters at pixel scale {}", multiple, pixels);
				return bucket;
			} catch (Exception | LinkageError e) {
				if (provider != null) provider.close();
				if (face != null) {
					synchronized (FreeTypeUtil.LIBRARY_LOCK) {
						FreeType.FT_Done_Face(face);
					}
				}
				if (font != null) {
					MemoryUtil.memFree(font);
				}
				ArcticMod.LOG.warn("smooth font (x{} letters): {}", multiple, e.toString());
				return null;
			}
		}

		BakedGlyph glyph(int codepoint, BakedGlyph base) {
			Object known = glyphs.get(codepoint);
			if (known instanceof Swapped swapped) {
				return swapped.base == base ? swapped : remember(codepoint, new Swapped(base, swapped.big));
			}
			if (known == NONE || glyphs.size() >= MAX_GLYPHS) {
				return base;
			}
			BakedGlyph big = bake(codepoint);
			if (big == null) {
				glyphs.put(codepoint, NONE);
				return base;
			}
			return remember(codepoint, new Swapped(base, big));
		}

		private Swapped remember(int codepoint, Swapped swapped) {
			glyphs.put(codepoint, swapped);
			return swapped;
		}

		private BakedGlyph bake(int codepoint) {
			if (!supported.contains(codepoint)) {
				return null;
			}
			baking = true;
			try {
				UnbakedGlyph unbaked = provider.getGlyph(codepoint);
				return unbaked == null ? null : unbaked.bake(this);
			} catch (RuntimeException e) {
				return null;
			} finally {
				baking = false;
			}
		}

		@Override
		public BakedGlyph stitch(GlyphInfo info, GlyphBitmap bitmap) {
			return stitcher.stitch(info, bitmap);
		}

		/** A letter this bucket can't make: the GUI-size one is drawn instead. */
		@Override
		public BakedGlyph getMissing() {
			return null;
		}

		void close() {
			glyphs.clear();
			stitcher.close();
			provider.close();
		}
	}

	/** The GUI-size glyph's metrics with the bucket glyph's look. */
	private static final class Swapped implements BakedGlyph {
		final BakedGlyph base;
		final BakedGlyph big;

		Swapped(BakedGlyph base, BakedGlyph big) {
			this.base = base;
			this.big = big;
		}

		@Override
		public GlyphInfo info() {
			return base.info();
		}

		@Override
		//#if MC >= 1.21.11
		public TextRenderable.Styled createGlyph(float x, float y, int color, int shadowColor, Style style, float boldOffset, float shadowOffset) {
		//#else
		public TextRenderable createGlyph(float x, float y, int color, int shadowColor, Style style, float boldOffset, float shadowOffset) {
		//#endif
			return big.createGlyph(x, y, color, shadowColor, style, boldOffset, shadowOffset);
		}
	}

	/** Inter from the mod jar, in native memory. */
	private static ByteBuffer read() {
		try (InputStream in = ScaledInter.class.getResourceAsStream(FILE)) {
			if (in == null) {
				ArcticMod.LOG.warn("smooth font: {} is missing from the mod", FILE);
				return null;
			}
			byte[] bytes = in.readAllBytes();
			ByteBuffer buffer = MemoryUtil.memAlloc(bytes.length);
			buffer.put(bytes).flip();
			return buffer;
		} catch (java.io.IOException e) {
			ArcticMod.LOG.warn("smooth font: {}", e.toString());
			return null;
		}
	}
}
//#endif
