package com.arcticlauncher.mod;

//#if MC >= 26.1
import java.awt.Font;
import java.awt.font.FontRenderContext;
import java.awt.font.LineMetrics;
import java.io.InputStream;

/**
 * Inter's line box, measured from the font file (ascent, descent and leading) at the same em the
 * chat font is drawn at. Chat used to space lines by a hardcoded 9 while the letters are this tall.
 */
public final class FancyFontMetrics {
	/** Em size in GUI pixels, the {@code size} in the smooth-font pack. */
	public static final float EM = 8.5f;
	private static final String FILE = "/resourcepacks/smooth_font/assets/arctic/font/inter.ttf";
	private static int line = -1;

	private FancyFontMetrics() {}

	/** Whole GUI pixels from one line's top to the next, at least the vanilla 9. */
	public static int lineHeight() {
		if (line < 0) {
			line = measure();
		}
		return line;
	}

	private static int measure() {
		try (InputStream in = FancyFontMetrics.class.getResourceAsStream(FILE)) {
			if (in == null) {
				return 9;
			}
			Font font = Font.createFont(Font.TRUETYPE_FONT, in).deriveFont(EM);
			LineMetrics metrics = font.getLineMetrics("Agjpq[]|", new FontRenderContext(null, true, true));
			return Math.max(9, Math.round(metrics.getHeight()));
		} catch (Exception e) {
			return 9;
		}
	}
}
//#endif
