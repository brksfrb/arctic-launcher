package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.VanillaMove;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Background fills, so the scoreboard's box is known (see VanillaMove). */
//#if MC >= 26.1
@Mixin(net.minecraft.client.gui.GuiGraphicsExtractor.class)
//#elif MC >= 1.20
@Mixin(net.minecraft.client.gui.GuiGraphics.class)
//#else
@Mixin(net.minecraft.client.gui.GuiComponent.class)
//#endif
abstract class FillCaptureMixin {
	//#if MC >= 1.20
	@Inject(method = "fill(IIIII)V", at = @At("HEAD"))
	private void arctic$filled(int x0, int y0, int x1, int y1, int color, CallbackInfo ci) {
		VanillaMove.filled(x0, y0, x1, y1);
	}
	//#elif MC >= 1.16
	@Inject(method = "fill(Lcom/mojang/blaze3d/vertex/PoseStack;IIIII)V", at = @At("HEAD"))
	private static void arctic$filled(com.mojang.blaze3d.vertex.PoseStack pose, int x0, int y0, int x1, int y1, int color,
			CallbackInfo ci) {
		VanillaMove.filled(x0, y0, x1, y1);
	}
	//#else
	@Inject(method = "fill(IIIII)V", at = @At("HEAD"))
	private static void arctic$filled(int x0, int y0, int x1, int y1, int color, CallbackInfo ci) {
		VanillaMove.filled(x0, y0, x1, y1);
	}
	//#endif
}
