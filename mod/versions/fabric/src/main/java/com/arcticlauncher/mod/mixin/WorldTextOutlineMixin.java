package com.arcticlauncher.mod.mixin;

//#if MC >= 26.2
import com.arcticlauncher.mod.ScaledInter;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * The 8x outline of glow-ink sign text (and other outlined world text) takes its letters from the same size bucket
 * as the text on top of it. From 26.2 world text is prepared at its on-screen size (see {@link WorldTextScaleMixin}):
 * the text's letters come from a sharp bucket through {@link ScaledGlyphMixin}, but the outline looks its letters up
 * itself and hands them to the builder already baked, past that hook. Close up, eight copies of the GUI-size letter,
 * magnified, then showed as a smeared, doubled dark rim around crisp text. The swapped letter keeps the GUI-size
 * metrics, so the outline's offsets and advances are unchanged.
 */
@Mixin(Font.class)
abstract class WorldTextOutlineMixin {
	@WrapOperation(method = "lambda$prepare8xTextOutline$0", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Font;getGlyph(ILnet/minecraft/network/chat/Style;)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;"))
	private BakedGlyph arctic$outlineGlyph(Font font, int codepoint, Style style, Operation<BakedGlyph> original) {
		return ScaledInter.glyph(codepoint, style, original.call(font, codepoint, style));
	}
}
//#endif
