package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Mob Size: every creature but the players is drawn bigger or smaller (hitboxes are unchanged). */
@Mixin(LivingEntityRenderer.class)
abstract class MobSizeLegacyMixin {
	@Inject(method = "scale", at = @At("HEAD"))
	private void arctic$size(LivingEntity entity, float partial, CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		if (c != null && c.mobSize && !(entity instanceof PlayerEntity)) {
			float f = (float) Math.max(0.25, Math.min(3.0, c.mobSizeFactor));
			org.lwjgl.opengl.GL11.glScalef(f, f, f);
		}
	}
}
