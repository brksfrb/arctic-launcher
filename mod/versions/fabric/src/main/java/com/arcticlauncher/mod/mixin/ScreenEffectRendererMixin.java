package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.ScreenEffectRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Low fire: the burning overlay sits lower so you can see. */
@Mixin(ScreenEffectRenderer.class)
abstract class ScreenEffectRendererMixin {
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
}
//#endif
