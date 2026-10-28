package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.21.2
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
//#if MC >= 1.21.9
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
//#else
import net.minecraft.client.renderer.entity.state.PlayerRenderState;
//#endif
import com.mojang.blaze3d.vertex.PoseStack;
//#elif MC >= 1.15
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
//#else
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
//#endif

/** Mob Size: every creature but the players is drawn bigger or smaller (hitboxes are unchanged). */
@Mixin(LivingEntityRenderer.class)
abstract class MobSizeMixin {
	//#if MC >= 1.21.2
	@Inject(method = "scale", at = @At("HEAD"))
	private void arctic$size(LivingEntityRenderState state, PoseStack pose, CallbackInfo ci) {
		float f = factor();
		//#if MC >= 1.21.9
		if (f != 1f && !(state instanceof AvatarRenderState)) {
		//#else
		if (f != 1f && !(state instanceof PlayerRenderState)) {
		//#endif
			pose.scale(f, f, f);
		}
	}
	//#elif MC >= 1.15
	@Inject(method = "scale", at = @At("HEAD"))
	private void arctic$size(LivingEntity entity, PoseStack pose, float partial, CallbackInfo ci) {
		float f = factor();
		if (f != 1f && !(entity instanceof Player)) {
			pose.scale(f, f, f);
		}
	}
	//#else
	@Inject(method = "scale", at = @At("HEAD"))
	private void arctic$size(LivingEntity entity, float partial, CallbackInfo ci) {
		float f = factor();
		if (f != 1f && !(entity instanceof Player)) {
			com.mojang.blaze3d.platform.GlStateManager.scalef(f, f, f);
		}
	}
	//#endif

	private static float factor() {
		ClientConfig c = ArcticClient.config();
		return c != null && c.mobSize ? (float) Math.max(0.25, Math.min(3.0, c.mobSizeFactor)) : 1f;
	}
}
