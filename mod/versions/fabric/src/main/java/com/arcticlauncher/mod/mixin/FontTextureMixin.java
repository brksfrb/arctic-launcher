package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9
//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
//#endif
import com.arcticlauncher.mod.ScaledInter;
//#if MC >= 26.3
import com.mojang.renderpearl.api.textures.FilterMode;
//#else
import com.mojang.blaze3d.textures.FilterMode;
//#endif
import net.minecraft.client.gui.font.FontTexture;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * With the smooth font on (26.1+), glyph textures scale smoothly (linear) instead
 * of pixel-sharp (nearest), so the oversampled TrueType glyphs look clean
 * at any GUI scale. Font textures are rebuilt when resources reload. The
 * smooth font's size buckets (see {@link ScaledInter}), which are always
 * drawn shrunk, filter linearly on every version from 1.21.9.
 */
@Mixin(FontTexture.class)
abstract class FontTextureMixin {
	//#if MC >= 26.3
	private static final String NEAREST = "Lcom/mojang/renderpearl/api/textures/FilterMode;NEAREST:Lcom/mojang/renderpearl/api/textures/FilterMode;";
	//#else
	private static final String NEAREST = "Lcom/mojang/blaze3d/textures/FilterMode;NEAREST:Lcom/mojang/blaze3d/textures/FilterMode;";
	//#endif

	@Redirect(method = "<init>", at = @At(value = "FIELD", target = NEAREST))
	private FilterMode arctic$filter() {
		//#if MC >= 26.1
		return ScaledInter.baking() || ArcticPacks.smoothFontOn() ? FilterMode.LINEAR : FilterMode.NEAREST;
		//#else
		return ScaledInter.baking() ? FilterMode.LINEAR : FilterMode.NEAREST;
		//#endif
	}
}
//#endif
