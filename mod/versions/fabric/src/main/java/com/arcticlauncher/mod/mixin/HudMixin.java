package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 1.21
import net.minecraft.client.DeltaTracker;
//#endif
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

/** Draws the Arctic HUD over the vanilla one (26.2 split the HUD out of Gui). */
//#if MC >= 26.2
@Mixin(Hud.class)
//#else
@Mixin(Gui.class)
//#endif
abstract class HudMixin {
	@Inject(method = "extractRenderState", at = @At("TAIL"))
	//#if MC >= 1.21
	private void arctic$hud(GuiGraphicsExtractor g, DeltaTracker delta, CallbackInfo ci) {
		ArcticClient.renderHud(new GfxImpl(g));
	}
	//#elif MC >= 1.20
	// Before 1.21 the HUD is drawn with a plain partial-tick float.
	private void arctic$hud(GuiGraphicsExtractor g, float delta, CallbackInfo ci) {
		ArcticClient.renderHud(new GfxImpl(g));
	}
	//#elif MC >= 1.16
	// Before 1.20: a PoseStack instead of GuiGraphics.
	private void arctic$hud(com.mojang.blaze3d.vertex.PoseStack pose, float delta, CallbackInfo ci) {
		ArcticClient.renderHud(GfxImpl.of(pose));
	}
	//#else
	// Before 1.16: not even a PoseStack (the global matrix).
	private void arctic$hud(float delta, CallbackInfo ci) {
		ArcticClient.renderHud(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()));
	}
	//#endif
}
