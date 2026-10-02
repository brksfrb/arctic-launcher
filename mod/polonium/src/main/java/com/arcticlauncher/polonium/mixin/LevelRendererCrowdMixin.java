package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.Crowd;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The crowd path ({@link Crowd}) covers the level's entities: it starts with them and hands in its submits once they're all in. */
@Mixin(LevelRenderer.class)
abstract class LevelRendererCrowdMixin {
	@Inject(method = "submitEntities", at = @At("HEAD"))
	private void polonium$beginCrowd(PoseStack poseStack, LevelRenderState levelRenderState, SubmitNodeCollector output, CallbackInfo ci) {
		Crowd.beginFrame();
	}

	@Inject(method = "submitEntities", at = @At("TAIL"))
	private void polonium$endCrowd(PoseStack poseStack, LevelRenderState levelRenderState, SubmitNodeCollector output, CallbackInfo ci) {
		Crowd.endSubmits(output);
	}
}
