package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 1.21
import net.minecraft.client.DeltaTracker;
//#endif
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#if MC >= 26.2
import net.minecraft.client.gui.Hud;
//#else
import net.minecraft.client.gui.Gui;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The custom crosshair, drawn instead of Minecraft's in first person. */
//#if MC >= 26.2
@Mixin(Hud.class)
//#else
@Mixin(Gui.class)
//#endif
abstract class HudCrosshairMixin {
	@Inject(
			//#if MC >= 26.1
			method = "extractCrosshair",
			//#else
			method = "renderCrosshair",
			//#endif
			at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.21
	private void arctic$crosshair(GuiGraphicsExtractor g, DeltaTracker delta, CallbackInfo ci) {
	//#elif MC >= 1.20.6
	private void arctic$crosshair(GuiGraphicsExtractor g, float delta, CallbackInfo ci) {
	//#elif MC >= 1.20
	private void arctic$crosshair(GuiGraphicsExtractor g, CallbackInfo ci) {
	//#elif MC >= 1.16
	// Before 1.20: a PoseStack instead of GuiGraphics.
	private void arctic$crosshair(com.mojang.blaze3d.vertex.PoseStack pose, CallbackInfo ci) {
		GfxImpl g = GfxImpl.of(pose);
	//#else
	// Before 1.16: nothing passed in (the global matrix).
	private void arctic$crosshair(CallbackInfo ci) {
		GfxImpl g = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
	//#endif
		Minecraft mc = Minecraft.getInstance();
		if (Compat.camera() != 0 || Compat.debugScreenShowing()) {
			return;
		}
		//#if MC >= 1.20
		if (ArcticClient.renderCrosshair(new GfxImpl(g))) {
		//#else
		if (ArcticClient.renderCrosshair(g)) {
		//#endif
			ci.cancel();
		}
	}
}
