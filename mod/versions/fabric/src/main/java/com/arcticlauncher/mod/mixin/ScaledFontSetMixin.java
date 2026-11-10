package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import com.arcticlauncher.mod.ScaledFont;
import com.arcticlauncher.mod.SmoothFont;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import com.llamalad7.mixinextras.injector.ModifyReturnValue;

@Mixin(FontSet.class)
abstract class ScaledFontSetMixin {
	@Shadow @Final private ResourceLocation name;

	//#if MC >= 1.16
	@ModifyReturnValue(method = "getGlyph(I)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;", at = @At("RETURN"))
	private BakedGlyph arctic$bucket(BakedGlyph original, int codepoint) {
	//#else
	@ModifyReturnValue(method = "getGlyph(C)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;", at = @At("RETURN"))
	private BakedGlyph arctic$bucket(BakedGlyph original, char codepoint) {
	//#endif
		return SmoothFont.on() && SmoothFont.isDefault(name)
				? ScaledFont.glyph(codepoint, original) : original;
	}
}
//#endif
