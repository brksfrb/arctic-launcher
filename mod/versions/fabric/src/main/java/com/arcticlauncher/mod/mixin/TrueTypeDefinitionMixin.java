package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import net.minecraft.client.gui.font.providers.TrueTypeGlyphProviderDefinition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/**
 * Rasterizes the smooth font at the GUI scale instead of a fixed oversample,
 * so glyphs are drawn 1:1 on screen: no blur from upscaling, no uneven
 * strokes from shrinking. The vertical shift is rounded to whole screen
 * pixels too, so FreeType's hinting (which lines stems up with pixels)
 * isn't undone by a fractional offset.
 */
@Mixin(TrueTypeGlyphProviderDefinition.class)
abstract class TrueTypeDefinitionMixin {
	@ModifyArgs(
			method = "load",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/font/TrueTypeGlyphProvider;<init>(Ljava/nio/ByteBuffer;Lorg/lwjgl/util/freetype/FT_Face;FFFFLjava/lang/String;)V"))
	private void arctic$pixelExact(Args args) {
		TrueTypeGlyphProviderDefinition self = (TrueTypeGlyphProviderDefinition) (Object) this;
		if (!ArcticPacks.isSmoothFont(self.location())) {
			return;
		}
		float scale = ArcticPacks.fontOversample();
		float shiftX = args.get(4);
		float shiftY = args.get(5);
		args.set(3, scale);
		args.set(4, Math.round(shiftX * scale) / scale);
		args.set(5, Math.round(shiftY * scale) / scale);
	}
}
//#endif
