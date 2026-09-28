package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.widget.OptionSliderWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Option sliders get the Arctic style's handle. */
@Mixin(OptionSliderWidget.class)
abstract class OptionSliderWidgetMixin {
	private static final int HANDLE_W = 8;

	@Redirect(method = "mouseDragged", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/OptionSliderWidget;drawTexture(IIIIII)V", ordinal = 0))
	private void arctic$handle(OptionSliderWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
			return;
		}
		Skin.sliderHandle(LegacyHooks.gfx(), ArcticClient.style(), x0, y0, HANDLE_W, h, self.isHovered());
	}

	@Redirect(method = "mouseDragged", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/OptionSliderWidget;drawTexture(IIIIII)V", ordinal = 1))
	private void arctic$handleRight(OptionSliderWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
		}
	}
}
