package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.vertex.PoseStack;
//#if MC < 26.2
import net.minecraft.client.Minecraft;
//#endif
//#if MC >= 26.2
import net.minecraft.client.renderer.SubmitNodeCollector;
//#elif MC >= 1.21.4
import net.minecraft.client.renderer.MultiBufferSource;
//#endif
import net.minecraft.client.renderer.ScreenEffectRenderer;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Low fire: the burning overlay sits lower so you can see. */
@Mixin(ScreenEffectRenderer.class)
abstract class ScreenEffectRendererMixin {
	//#if MC >= 26.2
	@Inject(method = "submitFire", at = @At("HEAD"))
	private static void arctic$lowerFire(PoseStack pose, SubmitNodeCollector nodes, TextureAtlasSprite sprite, CallbackInfo ci) {
		pose.pushPose();
		if (ArcticClient.config().lowFire) {
			pose.translate(0f, -0.3f, 0f);
		}
	}

	@Inject(method = "submitFire", at = @At("RETURN"))
	private static void arctic$restore(PoseStack pose, SubmitNodeCollector nodes, TextureAtlasSprite sprite, CallbackInfo ci) {
		pose.popPose();
	}
	//#elif MC >= 1.21.9
	@Inject(method = "renderFire", at = @At("HEAD"))
	private static void arctic$lowerFire(PoseStack pose, MultiBufferSource buffers, TextureAtlasSprite sprite, CallbackInfo ci) {
		pose.pushPose();
		if (ArcticClient.config().lowFire) {
			pose.translate(0f, -0.3f, 0f);
		}
	}

	@Inject(method = "renderFire", at = @At("RETURN"))
	private static void arctic$restore(PoseStack pose, MultiBufferSource buffers, TextureAtlasSprite sprite, CallbackInfo ci) {
		pose.popPose();
	}
	//#elif MC >= 1.21.4
	@Inject(method = "renderFire", at = @At("HEAD"))
	private static void arctic$lowerFire(PoseStack pose, MultiBufferSource buffers, CallbackInfo ci) {
		pose.pushPose();
		if (ArcticClient.config().lowFire) {
			pose.translate(0f, -0.3f, 0f);
		}
	}

	@Inject(method = "renderFire", at = @At("RETURN"))
	private static void arctic$restore(PoseStack pose, MultiBufferSource buffers, CallbackInfo ci) {
		pose.popPose();
	}
	//#else
	// Before 1.21.4 the fire overlay took the Minecraft instance first, then the pose.
	@Inject(method = "renderFire", at = @At("HEAD"))
	private static void arctic$lowerFire(Minecraft minecraft, PoseStack pose, CallbackInfo ci) {
		pose.pushPose();
		if (ArcticClient.config().lowFire) {
			pose.translate(0f, -0.3f, 0f);
		}
	}

	@Inject(method = "renderFire", at = @At("RETURN"))
	private static void arctic$restore(Minecraft minecraft, PoseStack pose, CallbackInfo ci) {
		pose.popPose();
	}
	//#endif
}
