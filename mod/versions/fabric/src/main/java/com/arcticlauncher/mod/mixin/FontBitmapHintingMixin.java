package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.platform.NativeImage;
import org.lwjgl.util.freetype.FT_Face;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Smooth font glyph images: at GUI sizes the regular weight's stems are
 * about 1.5 pixels, so each is one bright and one grey column. A slightly
 * heavier outline fills stems out and reads crisper.
 */
@Mixin(NativeImage.class)
abstract class FontBitmapHintingMixin {
	@WrapOperation(
			method = "copyFromFont",
			at = @At(value = "INVOKE", target = "Lorg/lwjgl/util/freetype/FreeType;FT_Load_Glyph(Lorg/lwjgl/util/freetype/FT_Face;II)I"))
	private int arctic$heavier(FT_Face face, int glyph, int flags, Operation<Integer> original) {
		return ArcticPacks.loadGlyph(face, glyph, flags, original::call);
	}
}
//#endif
