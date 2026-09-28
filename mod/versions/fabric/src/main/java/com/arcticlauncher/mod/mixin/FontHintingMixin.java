package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.font.TrueTypeGlyphProvider;
import org.lwjgl.util.freetype.FT_Face;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Smooth font glyph sizes: measured from the same slightly heavier outline
 * that {@link FontBitmapHintingMixin} draws, so sizes match exactly.
 */
@Mixin(TrueTypeGlyphProvider.class)
abstract class FontHintingMixin {
	@WrapOperation(
			method = "loadGlyph",
			at = @At(value = "INVOKE", target = "Lorg/lwjgl/util/freetype/FreeType;FT_Load_Glyph(Lorg/lwjgl/util/freetype/FT_Face;II)I"))
	private int arctic$heavier(FT_Face face, int glyph, int flags, Operation<Integer> original) {
		return ArcticPacks.loadGlyph(face, glyph, flags, original::call);
	}
}
//#endif
