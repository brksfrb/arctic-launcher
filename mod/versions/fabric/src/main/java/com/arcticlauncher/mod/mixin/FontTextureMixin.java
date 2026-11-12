package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.5
//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
//#endif
//#if MC >= 1.21.9
import com.arcticlauncher.mod.ScaledInter;
//#else
import com.arcticlauncher.mod.ScaledFont;
//#endif
//#if MC >= 26.3
import com.mojang.renderpearl.api.textures.FilterMode;
//#else
import com.mojang.blaze3d.textures.FilterMode;
//#endif
import net.minecraft.client.gui.font.FontTexture;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
//#if MC >= 26.2 && MC < 26.3
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;
//#endif

/**
 * With the smooth font on (26.1+), glyph textures scale smoothly (linear) instead
 * of pixel-sharp (nearest), so the oversampled TrueType glyphs look clean
 * at any GUI scale. Font textures are rebuilt when resources reload. The
 * smooth font's size buckets (see {@link com.arcticlauncher.mod.TextDrawScale}), which are always
 * drawn shrunk, filter linearly on every version from 1.21.5.
 */
@Mixin(FontTexture.class)
abstract class FontTextureMixin {
	//#if MC >= 26.3
	private static final String NEAREST = "Lcom/mojang/renderpearl/api/textures/FilterMode;NEAREST:Lcom/mojang/renderpearl/api/textures/FilterMode;";
	//#else
	private static final String NEAREST = "Lcom/mojang/blaze3d/textures/FilterMode;NEAREST:Lcom/mojang/blaze3d/textures/FilterMode;";
	//#endif

	/** This texture filters linearly: its letters are sampled between texels. */
	@Unique private boolean arctic$linear;

	@Redirect(method = "<init>", at = @At(value = "FIELD", target = NEAREST))
	private FilterMode arctic$filter() {
		//#if MC >= 26.1
		FilterMode mode = ScaledInter.baking() || ArcticPacks.smoothFontOn() ? FilterMode.LINEAR : FilterMode.NEAREST;
		//#elif MC >= 1.21.9
		FilterMode mode = ScaledInter.baking() ? FilterMode.LINEAR : FilterMode.NEAREST;
		//#else
		FilterMode mode = ScaledFont.baking() ? FilterMode.LINEAR : FilterMode.NEAREST;
		//#endif
		arctic$linear = mode == FilterMode.LINEAR;
		return mode;
	}

	//#if MC >= 26.2 && MC < 26.3
	/**
	 * A linear filter blends the four nearest texels, so a letter's border pixels also read what lies just outside its
	 * rectangle in the atlas (the game packs the rectangles 1 texel apart, and its texture coordinates sit on the
	 * rectangle's edge): that shows as thin lines at a letter's left and right edges when it is magnified. Taking half a
	 * texel off each side, of the texture coordinates and the drawn quad alike (same letter, same place), makes the
	 * border pixels read the letter's own outermost texel only.
	 */
	@ModifyArgs(method = "add", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedSheetGlyph;<init>(Lcom/mojang/blaze3d/font/GlyphInfo;Lnet/minecraft/client/gui/font/GlyphRenderTypes;Lcom/mojang/blaze3d/textures/GpuTextureView;FFFFFFFF)V"))
	private void arctic$inset(Args args) {
		if (!arctic$linear) {
			return;
		}
		float u0 = args.get(3), u1 = args.get(4), v0 = args.get(5), v1 = args.get(6);
		float left = args.get(7), right = args.get(8), top = args.get(9), bottom = args.get(10);
		float width = (u1 - u0) * 256f;
		float height = (v1 - v0) * 256f;
		if (width < 2f || height < 2f) {
			return;
		}
		float half = 0.5f / 256f;
		float dx = 0.5f * (right - left) / width;
		float dy = 0.5f * (bottom - top) / height;
		args.set(3, u0 + half);
		args.set(4, u1 - half);
		args.set(5, v0 + half);
		args.set(6, v1 - half);
		args.set(7, left + dx);
		args.set(8, right - dx);
		args.set(9, top + dy);
		args.set(10, bottom - dy);
	}
	//#endif
}
//#endif
