package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import com.mojang.blaze3d.font.GlyphInfo;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * With the smooth font, shadows and bold sit about half a GUI pixel off
 * (rounded to whole screen pixels) instead of a whole one: a full-pixel
 * shadow makes thin anti-aliased letters look doubled and blocky.
 */
@Mixin(targets = "com.mojang.blaze3d.font.TrueTypeGlyphProvider$Glyph")
abstract class TrueTypeGlyphMixin {
	@Redirect(
			method = "<init>",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/font/GlyphInfo;simple(F)Lcom/mojang/blaze3d/font/GlyphInfo;"))
	private GlyphInfo arctic$info(float advance) {
		if (!ArcticPacks.smoothFontOn()) {
			return GlyphInfo.simple(advance);
		}
		float offset = ArcticPacks.shadowOffset();
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
