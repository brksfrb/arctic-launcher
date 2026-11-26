//#if MC < 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.GfxImpl;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.OptionSliderWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** 1.13 option sliders: the Arctic handle (two blits in the game's own handle drawing: the first draws it, the second is dropped). */
@Mixin(OptionSliderWidget.class)
abstract class OptionSliderWidget13Mixin {
	private static final int HANDLE_W = 8;

	@WrapOperation(method = "method_893", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/OptionSliderWidget;blit(IIIIII)V", ordinal = 0))
	private void arctic$handle(OptionSliderWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
			return;
		}
		Skin.sliderHandle(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), ArcticClient.style(), x0, y0, HANDLE_W, h,
				Compat.widgetHovered((ButtonWidget) (Object) self));
	}

	@WrapOperation(method = "method_893", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/OptionSliderWidget;blit(IIIIII)V", ordinal = 1))
	private void arctic$handleRight(OptionSliderWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
		}
	}
}
//#endif
