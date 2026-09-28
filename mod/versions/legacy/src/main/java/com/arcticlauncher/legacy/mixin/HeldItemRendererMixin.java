package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.item.HeldItemRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Low fire: the burning overlay sits lower on the screen. */
@Mixin(HeldItemRenderer.class)
abstract class HeldItemRendererMixin {
	private static final float LOWER = -0.3f;
	//#if MC >= 1.12
	private static final String FIRE = "method_14680";
	//#else
	private static final String FIRE = "renderFireOverlay";
	//#endif

	@Inject(method = FIRE, at = @At("HEAD"))
	private void arctic$lowerStart(CallbackInfo ci) {
		GlStateManager.pushMatrix();
		if (ArcticClient.config().lowFire) {
			GlStateManager.translate(0f, LOWER, 0f);
		}
	}

	@Inject(method = FIRE, at = @At("RETURN"))
	private void arctic$lowerEnd(CallbackInfo ci) {
		GlStateManager.popMatrix();
	}
}
