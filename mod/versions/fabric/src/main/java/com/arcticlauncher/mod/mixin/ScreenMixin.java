package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.mod.GfxImpl;
import com.arcticlauncher.mod.VulkanCompat;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Menus outside a world show the Arctic night instead of the panorama (1.20.6+) or the dirt
 * background (before; the title screen's own panorama is TitleScreenMixin's).
 */
@Mixin(Screen.class)
abstract class ScreenMixin {
	@Inject(method = "extractRenderState", at = @At("HEAD"))
	//#if MC >= 1.20
	private void arctic$layout(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
	//#elif MC >= 1.16
	private void arctic$layout(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
	//#else
	private void arctic$layout(int mouseX, int mouseY, float delta, CallbackInfo ci) {
	//#endif
		com.arcticlauncher.mod.ArcticButton.layout((Screen) (Object) this);
	}

	//#if MC >= 1.20.6
	@Inject(method = "extractPanorama", at = @At("HEAD"), cancellable = true)
	private void arctic$backdrop(GuiGraphicsExtractor g, float delta, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g), ci);
	}
	//#elif MC >= 1.20
	@Inject(method = "renderDirtBackground", at = @At("HEAD"), cancellable = true)
	private void arctic$backdrop(GuiGraphicsExtractor g, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g), ci);
	}
	//#elif MC >= 1.19.4
	@Inject(method = "renderDirtBackground", at = @At("HEAD"), cancellable = true)
	private void arctic$backdrop(com.mojang.blaze3d.vertex.PoseStack pose, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(pose), ci);
	}
	//#else
	// Before 1.19.4 the dirt is drawn with the global matrix, offset by an int.
	@Inject(method = "renderDirtBackground", at = @At("HEAD"), cancellable = true)
	private void arctic$backdrop(int offset, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), ci);
	}
	//#endif

	private static void arctic$draw(GfxImpl gfx, CallbackInfo ci) {
		if (ArcticClient.restyles()) {
			Backdrop.render(gfx, ArcticClient.style());
			// Under VulkanMod the game's panorama still draws (hidden under the backdrop): VulkanMod's
			// menu blur crashes the graphics driver when nothing was drawn into the frame before it.
			if (!VulkanCompat.ACTIVE) {
				ci.cancel();
			}
		}
	}
}
