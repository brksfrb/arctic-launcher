package com.arcticlauncher.mod.mixin;

//#if MC >= 1.20.6
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Menus outside a world show the Arctic night instead of the panorama.
 * Before 1.20.6 there's no generic Screen hook for this (see TitleScreenMixin).
 */
@Mixin(Screen.class)
abstract class ScreenMixin {
	@Inject(method = "extractPanorama", at = @At("HEAD"), cancellable = true)
	private void arctic$backdrop(GuiGraphicsExtractor g, float delta, CallbackInfo ci) {
		if (ArcticClient.restyles()) {
			Backdrop.render(new GfxImpl(g), ArcticClient.style());
			ci.cancel();
		}
	}
}
//#endif
