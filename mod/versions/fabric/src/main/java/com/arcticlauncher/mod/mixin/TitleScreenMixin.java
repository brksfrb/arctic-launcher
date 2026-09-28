package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticButton;
//#if MC < 1.20.6
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.mod.GfxImpl;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.PanoramaRenderer;
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Adds the Arctic button to the title screen. */
@Mixin(TitleScreen.class)
abstract class TitleScreenMixin extends Screen {
	private TitleScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		addRenderableWidget(ArcticButton.create());
	}

	//#if MC < 1.20.6
	// Before 1.20.6 there's no generic Screen.renderPanorama hook (see ScreenMixin):
	// only the title screen draws one, directly, so redirect that call instead.
	@Redirect(method = "extractRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/PanoramaRenderer;render(FF)V"))
	//#if MC >= 1.20
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha,
			@Local(argsOnly = true) GuiGraphicsExtractor g) {
		GfxImpl gfx = new GfxImpl(g);
	//#elif MC >= 1.16
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha,
			@Local(argsOnly = true) com.mojang.blaze3d.vertex.PoseStack pose) {
		GfxImpl gfx = GfxImpl.of(pose);
	//#else
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha) {
		GfxImpl gfx = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
	//#endif
		if (ArcticClient.restyles()) {
			Backdrop.render(gfx, ArcticClient.style());
		} else {
			panorama.render(partialTick, alpha);
		}
	}
	//#endif
}
