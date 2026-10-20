//#if MC < 1.15
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.renderer.ItemInHandRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Low fire on 1.14, where the burning overlay is drawn by the hand renderer (see ScreenEffectRendererMixin). */
@Mixin(ItemInHandRenderer.class)
abstract class FireOldMixin {
	@Inject(method = "renderFire()V", at = @At("HEAD"))
	private void arctic$lowerFire(CallbackInfo ci) {
		GlStateManager.pushMatrix();
		if (ArcticClient.config().lowFire) {
			GlStateManager.translatef(0f, -0.3f, 0f);
		}
	}

	@Inject(method = "renderFire()V", at = @At("RETURN"))
	private void arctic$restore(CallbackInfo ci) {
		GlStateManager.popMatrix();
	}
}
//#endif
