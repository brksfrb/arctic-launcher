package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.DeltaTracker;
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
	private void arctic$hud(GuiGraphicsExtractor g, DeltaTracker delta, CallbackInfo ci) {
		ArcticClient.renderHud(new GfxImpl(g));
	}
}
