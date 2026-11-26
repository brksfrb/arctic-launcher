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

/**
 * 1.13: every vanilla button gets the Arctic style's background (sliders a track). The one widget class draws
 * its background as two blits (left half, right half) of widgets.png: the first draws the whole Arctic
 * widget, the second is dropped.
 */
@Mixin(ButtonWidget.class)
abstract class ButtonWidget13Mixin {
	@org.spongepowered.asm.mixin.Shadow
	public int x;
	@org.spongepowered.asm.mixin.Shadow
	public int y;
	@org.spongepowered.asm.mixin.Shadow
	protected int width;
	@org.spongepowered.asm.mixin.Shadow
	protected int height;

	@WrapOperation(method = "method_891", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;blit(IIIIII)V", ordinal = 0))
	private void arctic$background(ButtonWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
			return;
		}
		GfxImpl gfx = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
		if (isSlider(self)) {
			Skin.sliderTrack(gfx, ArcticClient.style(), x, y, width, height, Compat.widgetHovered(self));
		} else {
			Skin.button(gfx, ArcticClient.style(), x, y, width, height, Compat.widgetActive(self),
					Compat.widgetHovered(self) ? 1f : 0f);
		}
	}

	@WrapOperation(method = "method_891", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;blit(IIIIII)V", ordinal = 1))
	private void arctic$backgroundRight(ButtonWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
		}
	}

	/** Option sliders and the volume sliders (a package-private subclass of the one widget class). */
	private static boolean isSlider(ButtonWidget w) {
		return w instanceof OptionSliderWidget || w instanceof com.arcticlauncher.mod.SliderMark13;
	}
}
//#endif
