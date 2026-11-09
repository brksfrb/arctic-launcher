package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.5
import com.arcticlauncher.mod.ScaledFont;
import net.minecraft.client.gui.font.FontTexture;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(FontTexture.class)
abstract class ScaledFontTextureMixin {
	@Inject(method = "add", at = @At("RETURN"))
	private void arctic$linear(CallbackInfoReturnable<BakedGlyph> cir) {
		if (ScaledFont.baking() && cir.getReturnValue() != null) {
			((FontTexture) (Object) this).setFilter(true, false);
		}
	}
}
//#endif
