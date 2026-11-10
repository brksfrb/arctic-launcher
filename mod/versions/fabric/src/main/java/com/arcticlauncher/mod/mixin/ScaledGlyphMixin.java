package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9
import com.arcticlauncher.mod.ScaledInter;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Each character of text being drawn: scaled-up smooth-font letters come from a size bucket (see
 * {@link ScaledInter}). Widths are measured elsewhere and never pass through here.
 */
@Mixin(targets = "net.minecraft.client.gui.Font$PreparedTextBuilder")
abstract class ScaledGlyphMixin {
	@ModifyExpressionValue(
			method = "accept(ILnet/minecraft/network/chat/Style;I)Z",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/Font;getGlyph(ILnet/minecraft/network/chat/Style;)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;"))
	private BakedGlyph arctic$scaled(BakedGlyph original, int index, Style style, int codepoint) {
		return ScaledInter.glyph(codepoint, style, original);
	}
}
//#endif
