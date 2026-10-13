package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Coerce;
import org.spongepowered.asm.mixin.injection.Redirect;

/** The volume sliders (Music & Sounds) get the Arctic style's handle too (as {@link OptionSliderWidgetMixin}). */
@Mixin(targets = "net.minecraft.client.gui.screen.SoundsScreen$SoundButtonWidget")
abstract class SoundSliderWidgetMixin {
	// The class isn't public, so the handle takes it as a button; optional (require = 0): if that ever
	// doesn't apply, the slider keeps the game's own handle instead of the game failing to start.
	private static final int HANDLE_W = 8;

	@Redirect(method = "mouseDragged", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/SoundsScreen$SoundButtonWidget;drawTexture(IIIIII)V", ordinal = 0), require = 0)
	private void arctic$handle(@Coerce ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
			return;
		}
		Skin.sliderHandle(LegacyHooks.gfx(), ArcticClient.style(), x0, y0, HANDLE_W, h, self.isHovered());
	}

	@Redirect(method = "mouseDragged", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/SoundsScreen$SoundButtonWidget;drawTexture(IIIIII)V", ordinal = 1), require = 0)
	private void arctic$handleRight(@Coerce ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
		}
	}
}
