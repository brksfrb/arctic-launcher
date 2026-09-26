package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The custom crosshair, drawn instead of Minecraft's in first person. */
@Mixin(Hud.class)
abstract class HudCrosshairMixin {
	@Inject(method = "extractCrosshair", at = @At("HEAD"), cancellable = true)
	private void arctic$crosshair(GuiGraphicsExtractor g, DeltaTracker delta, CallbackInfo ci) {
		Minecraft mc = Minecraft.getInstance();
		if (!mc.options.getCameraType().isFirstPerson() || mc.getDebugOverlay().showDebugScreen()) {
			return;
		}
		if (ArcticClient.renderCrosshair(new GfxImpl(g))) {
			ci.cancel();
		}
	}
}
//#endif
