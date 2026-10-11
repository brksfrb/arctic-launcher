package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyVanillaMove;
import net.minecraft.client.gui.DrawableHelper;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Background fills, so the scoreboard's box is known (see LegacyVanillaMove). */
@Mixin(DrawableHelper.class)
abstract class FillCaptureMixin {
	@Inject(method = "fill(IIIII)V", at = @At("HEAD"))
	private static void arctic$filled(int x0, int y0, int x1, int y1, int color, CallbackInfo ci) {
		LegacyVanillaMove.filled(x0, y0, x1, y1);
	}
}
