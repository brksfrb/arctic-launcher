package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import com.arcticlauncher.mod.SmoothFont;
import org.spongepowered.asm.mixin.Mixin;

/**
 * With the smooth font, TrueType text shadows and bold sit about half a GUI pixel off instead of
 * a whole one (see {@link SmoothFont#shadowOffset}). These override the glyph interface's defaults.
 */
@Mixin(targets = "com.mojang.blaze3d.font.TrueTypeGlyphProvider$Glyph")
abstract class SmoothFontGlyphMixin {
	public float getShadowOffset() {
		return SmoothFont.on() ? SmoothFont.shadowOffset() : 1.0f;
	}

	public float getBoldOffset() {
		return SmoothFont.on() ? SmoothFont.shadowOffset() : 1.0f;
	}
}
//#endif
