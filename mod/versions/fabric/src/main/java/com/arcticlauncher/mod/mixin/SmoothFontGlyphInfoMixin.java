package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9 && MC < 26.1
import com.arcticlauncher.mod.SmoothFont;
import com.mojang.blaze3d.font.GlyphInfo;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * With the smooth font on 1.21.9 - 1.21.11, TrueType text shadows and bold sit about half a GUI
 * pixel off instead of a whole one (see {@link SmoothFont#shadowOffset}).
 */
@Mixin(targets = "com.mojang.blaze3d.font.TrueTypeGlyphProvider$Glyph")
abstract class SmoothFontGlyphInfoMixin {
	@Redirect(
			method = "<init>",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/font/GlyphInfo;simple(F)Lcom/mojang/blaze3d/font/GlyphInfo;"))
	private GlyphInfo arctic$info(float advance) {
		if (!SmoothFont.on()) {
			return GlyphInfo.simple(advance);
		}
		float offset = SmoothFont.shadowOffset();
		return new GlyphInfo() {
			@Override
			public float getAdvance() {
				return advance;
			}

			@Override
			public float getBoldOffset() {
				return offset;
			}

			@Override
			public float getShadowOffset() {
				return offset;
			}
		};
	}
}
//#endif
