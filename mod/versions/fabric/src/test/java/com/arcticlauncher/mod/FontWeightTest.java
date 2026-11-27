package com.arcticlauncher.mod;

//#if MC >= 1.20.5
import org.junit.jupiter.api.Test;
import org.lwjgl.util.freetype.FreeType;

import static org.junit.jupiter.api.Assertions.assertEquals;

class FontWeightTest {
	private static final int NO_HINTS = FreeType.FT_LOAD_NO_HINTING | FreeType.FT_LOAD_NO_AUTOHINT;
	private static final int RENDER = FreeType.FT_LOAD_RENDER;
	private static final int AUTOHINT = FreeType.FT_LOAD_FORCE_AUTOHINT;

	@Test
	void vanillaGlyphLoadDisablesHintingAndKeepsRendering() {
		checkLoad(false, false, RENDER | NO_HINTS);
	}

	@Test
	void smoothGlyphLoadDisablesHintingWithOrWithoutSharpEdges() {
		checkLoad(true, false, FreeType.FT_LOAD_NO_BITMAP | NO_HINTS);
		checkLoad(true, true, FreeType.FT_LOAD_NO_BITMAP | NO_HINTS);
	}

	private static void checkLoad(boolean smooth, boolean sharp, int expectedFlags) {
		// A failing loader stops before outline handling. Capture the flags at the native-call boundary.
		int result = FontWeight.loadGlyph(null, 42, RENDER | AUTOHINT, (face, glyph, flags) -> {
			assertEquals(42, glyph);
			assertEquals(expectedFlags, flags);
			return 17;
		}, smooth, 1, sharp);
		assertEquals(17, result);
	}
}
//#endif
