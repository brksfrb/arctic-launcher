//#if MC < 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.GfxImpl;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Coerce;

/** 1.13 volume sliders (Music & Sounds): the Arctic handle too, as {@link OptionSliderWidget13Mixin}. */
@Mixin(targets = "net.minecraft.client.gui.screen.SoundsScreen$SoundButtonWidget")
abstract class SoundSliderWidget13Mixin implements com.arcticlauncher.mod.SliderMark13 {
	private static final int HANDLE_W = 8;

	// The class isn't public, so the handle takes it as a button; optional (require = 0): if that ever
	// doesn't apply, the slider keeps the game's own handle instead of the game failing to start.
	@WrapOperation(method = "method_893", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/SoundsScreen$SoundButtonWidget;blit(IIIIII)V", ordinal = 0), require = 0)
	private void arctic$handle(@Coerce ButtonWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
			return;
		}
		Skin.sliderHandle(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), ArcticClient.style(), x0, y0, HANDLE_W, h,
				Compat.widgetHovered(self));
	}

	@WrapOperation(method = "method_893", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/SoundsScreen$SoundButtonWidget;blit(IIIIII)V", ordinal = 1), require = 0)
	private void arctic$handleRight(@Coerce ButtonWidget self, int x0, int y0, int u, int v, int w, int h, Operation<Void> original) {
		if (!ArcticClient.restyles()) {
			original.call(self, x0, y0, u, v, w, h);
		}
	}
}
//#endif
