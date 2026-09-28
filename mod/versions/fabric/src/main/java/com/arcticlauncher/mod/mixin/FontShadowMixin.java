package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * With the smooth font, text shadows are softer: a solid shadow half a
 * pixel behind anti-aliased letters shows through their edges as gray and
 * makes them look doubled. The shadow keeps its tint at a fraction of its
 * opacity; shadows a text sets for itself are left alone.
 */
@Mixin(targets = "net.minecraft.client.gui.Font$PreparedTextBuilder")
abstract class FontShadowMixin {
	private static final float ARCTIC_SOFT_SHADOW = 0.5f;

	@Inject(method = "getShadowColor", at = @At("RETURN"), cancellable = true)
	private void arctic$soften(Style style, int color, CallbackInfoReturnable<Integer> cir) {
		int shadow = cir.getReturnValueI();
		if (shadow == 0 || style.getShadowColor() != null || !ArcticPacks.smoothFontCached()) {
			return;
		}
		int alpha = Math.round(((shadow >>> 24) & 0xFF) * ARCTIC_SOFT_SHADOW);
		cir.setReturnValue((alpha << 24) | (shadow & 0x00FFFFFF));
	}
}
//#endif
