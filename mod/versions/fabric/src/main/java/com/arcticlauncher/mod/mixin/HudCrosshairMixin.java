//#if MC >= 1.20
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
	//#else
	private void arctic$crosshair(GuiGraphicsExtractor g, CallbackInfo ci) {
	//#endif
		Minecraft mc = Minecraft.getInstance();
		if (!mc.options.getCameraType().isFirstPerson() || Compat.debugScreenShowing()) {
			return;
		}
		if (ArcticClient.renderCrosshair(new GfxImpl(g))) {
			ci.cancel();
		}
	}
}
//#endif
