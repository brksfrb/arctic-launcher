package com.arcticlauncher.mod;

//#if MC >= 1.20.5
import org.lwjgl.util.freetype.FT_Face;
import org.lwjgl.util.freetype.FT_GlyphSlot;
import org.lwjgl.util.freetype.FreeType;

/**
 * The smooth font's weight on FreeType (1.20.5+): at GUI sizes Inter's regular stems are about 1.5
 * pixels, one bright and one grey column, which reads thin. Glyphs are loaded with a slightly
 * heavier outline, the same for measuring and drawing so sizes agree (FontHintingMixin,
 * FontBitmapHintingMixin).
 */
public final class FontWeight {
	private static final int FT_LOAD_RENDER = 1 << 2;
	private static final int FT_LOAD_NO_BITMAP = 1 << 3;
	private static final int FT_LOAD_BITMAP_METRICS_ONLY = 1 << 22;
	private static final int FT_RENDER_MODE_NORMAL = 0;
	/** Extra stem weight in 1/64 pixel: sideways a little over a third of a pixel, a little upward. */
	private static final long EMBOLDEN_X = 24;
	private static final long EMBOLDEN_Y = 8;
	/** The smooth font's size in GUI pixels (its em). */
	private static final float FONT_SIZE = 8.5f;

	/** FT_Load_Glyph, as the game calls it. */
	public interface GlyphLoader {
		int load(FT_Face face, int glyph, int flags);
	}

	private FontWeight() {}

	public static int loadGlyph(FT_Face face, int glyph, int flags, GlyphLoader original) {
		//#if MC >= 26.1
		boolean on = ArcticPacks.smoothFontOn();
		int scale = ArcticPacks.loadedOversample();
		//#else
		boolean on = SmoothFont.on();
		int scale = SmoothFont.loadedScale();
		//#endif
		if (!on) {
			return original.load(face, glyph, flags);
		}
		int error = original.load(face, glyph, (flags & ~FT_LOAD_RENDER & ~FT_LOAD_BITMAP_METRICS_ONLY) | FT_LOAD_NO_BITMAP);
		if (error != 0) {
			return error;
		}
		FT_GlyphSlot slot = face.glyph();
		if (slot == null) {
			return error;
		}
		// The same weight at every size: titles are rasterized several times larger.
		float size = emboldenScale(face, scale);
		FreeType.FT_Outline_EmboldenXY(slot.outline(), Math.round(EMBOLDEN_X * size), Math.round(EMBOLDEN_Y * size));
		return FreeType.FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL);
	}

	/** How much larger than chat text this face is rasterized (1 for chat, 4 for titles). */
	private static float emboldenScale(FT_Face face, int loadedScale) {
		org.lwjgl.util.freetype.FT_Size size = face.size();
		if (size == null) {
			return 1;
		}
		return Math.max(1, size.metrics().x_ppem() / (FONT_SIZE * Math.max(1, loadedScale)));
	}
}
//#endif
