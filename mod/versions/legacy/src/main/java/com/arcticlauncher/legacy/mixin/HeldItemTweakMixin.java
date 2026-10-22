package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.HeldItemTweak;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.item.HeldItemRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Held Item: the hands are drawn inside a saved matrix with the player's offset and size applied. */
@Mixin(HeldItemRenderer.class)
abstract class HeldItemTweakMixin {
	@Inject(method = "renderArmHoldingItem", at = @At("HEAD"))
	private void arctic$start(float partial, CallbackInfo ci) {
		GlStateManager.pushMatrix();
		float[] v = HeldItemTweak.values(ArcticClient.config());
		if (v != null) {
			GlStateManager.translate(v[0], v[1], v[2]);
			GlStateManager.scale(v[3], v[3], v[3]);
		}
	}

	@Inject(method = "renderArmHoldingItem", at = @At("RETURN"))
	private void arctic$end(float partial, CallbackInfo ci) {
		GlStateManager.popMatrix();
	}
}
