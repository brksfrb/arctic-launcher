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
	private static final int FT_LOAD_NO_HINTING = 1 << 1;
	private static final int FT_LOAD_FORCE_AUTOHINT = 1 << 5;
	private static final int FT_LOAD_TARGET_MONO = 2 << 16;
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
		boolean sharp = !com.arcticlauncher.client.ArcticClient.config().fontAntialias;
		String variant = System.getProperty("arctic.sharp", "A");
		int load = (flags & ~FT_LOAD_RENDER & ~FT_LOAD_BITMAP_METRICS_ONLY) | FT_LOAD_NO_BITMAP;
		boolean hinted = sharp && !"A".equals(variant) && emboldenScaleOf(face, scale) <= 1.01f;
		if (hinted) {
			load = (load & ~FT_LOAD_NO_HINTING) | FT_LOAD_TARGET_MONO | ("C".equals(variant) ? FT_LOAD_FORCE_AUTOHINT : 0);
		}
		int error = original.load(face, glyph, load);
		if (error != 0) {
			return error;
		}
		FT_GlyphSlot slot = face.glyph();
		if (slot == null) {
			return error;
		}
		// The same weight at every size: titles are rasterized several times larger.
		float size = emboldenScale(face, scale);
		if (!hinted) {
			FreeType.FT_Outline_EmboldenXY(slot.outline(), Math.round(EMBOLDEN_X * size), Math.round(EMBOLDEN_Y * size));
		}
		int rendered = FreeType.FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL);
		// Chat-sized text is pixel-sharp unless anti-aliasing is asked for; titles (rasterized larger) stay smooth.
		if (rendered == 0 && size <= 1.01f && sharp) {
			sharpen(slot);
		}
		return rendered;
	}

	private static float emboldenScaleOf(FT_Face face, int scale) {
		return emboldenScale(face, scale);
	}

	/** Every pixel of the glyph fully on or off: no in-between shades. Below this much coverage a pixel is off. */
	private static final int SHARP_AT = 128;

	private static void sharpen(FT_GlyphSlot slot) {
		org.lwjgl.util.freetype.FT_Bitmap bitmap = slot.bitmap();
		int rows = bitmap.rows();
		int pitch = Math.abs(bitmap.pitch());
		if (rows == 0 || pitch == 0 || bitmap.pixel_mode() != FreeType.FT_PIXEL_MODE_GRAY) {
			return;
		}
		java.nio.ByteBuffer pixels = bitmap.buffer(rows * pitch);
		if (pixels == null) {
			return;
		}
		for (int i = 0; i < rows * pitch; i++) {
			pixels.put(i, (byte) ((pixels.get(i) & 0xFF) >= SHARP_AT ? 0xFF : 0));
		}
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
