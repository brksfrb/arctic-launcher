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
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(FontSet.class)
abstract class ScaledFontSetMixin {
	@Shadow @Final private ResourceLocation name;

	//#if MC >= 1.16
	@Inject(method = "getGlyph(I)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;", at = @At("RETURN"), cancellable = true)
	private void arctic$bucket(int codepoint, CallbackInfoReturnable<BakedGlyph> cir) {
	//#else
	@Inject(method = "getGlyph(C)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;", at = @At("RETURN"), cancellable = true)
	private void arctic$bucket(char codepoint, CallbackInfoReturnable<BakedGlyph> cir) {
	//#endif
		if (SmoothFont.isDefault(name)) {
			cir.setReturnValue(ScaledFont.glyph(codepoint, cir.getReturnValue()));
		}
	}
}
//#endif
