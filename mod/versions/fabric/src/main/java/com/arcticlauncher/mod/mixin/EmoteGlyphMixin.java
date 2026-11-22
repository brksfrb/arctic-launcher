package com.arcticlauncher.mod.mixin;

//#if MC < 1.19
import com.arcticlauncher.client.feature.EmoteAtlas;
import com.arcticlauncher.mod.EmoteFont;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Old font metrics and baked glyphs are separate; use the shared atlas for the baked quad. */
@Mixin(FontSet.class)
abstract class EmoteGlyphMixin {
	@Inject(method = "getGlyph", at = @At("HEAD"), cancellable = true)
	private void arctic$glyph(
			//#if MC >= 1.16
			int cp,
			//#else
			char cp,
			//#endif
			CallbackInfoReturnable<BakedGlyph> cir) {
		EmoteAtlas.Emote emote = EmoteAtlas.glyph(cp);
		if (emote != null) cir.setReturnValue(EmoteFont.baked(emote));
	}
}
//#endif
