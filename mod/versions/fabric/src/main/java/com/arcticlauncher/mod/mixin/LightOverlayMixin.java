package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.11
import net.minecraft.client.renderer.LevelRenderer;
//#elif MC >= 1.15
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.debug.DebugRenderer;
//#else
import net.minecraft.client.renderer.GameRenderer;
//#endif
//#if MC >= 1.21.2 && MC < 1.21.11
import net.minecraft.client.renderer.culling.Frustum;
//#endif
//#if MC >= 26.2
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.state.level.LevelRenderState;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Light Overlay ("Light" page, "Show on ground"): the markers drawn in each version's world pass, where the game draws
 * its own debug overlays (chunk borders), after the opaque terrain so they are depth tested against it.
 */
//#if MC >= 1.21.11
@Mixin(LevelRenderer.class)
//#elif MC >= 1.15
@Mixin(DebugRenderer.class)
//#else
@Mixin(GameRenderer.class)
//#endif
abstract class LightOverlayMixin {
	//#if MC >= 26.2
	/** 26.2+: submitted with the gizmos (right after the block outline) into this frame's features. */
	@Inject(method = "submitFeatures", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;finalizeGizmoCollection()V"), require = 0)
	private void arctic$lightOverlay(LevelRenderState state, SubmitNodeCollector collector, boolean outline, CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.submit(collector);
	}
	//#elif MC >= 1.21.11
	/** 1.21.11 - 26.1: right after the block outline (finalizing the gizmos is the next step there). */
	@Inject(method = "finalizeGizmoCollection", at = @At("HEAD"), require = 0)
	private void arctic$lightOverlay(CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.drawAtGizmos();
	}
	//#elif MC >= 1.21.9
	/** 1.21.9 - 1.21.10: with the opaque debug renderers (chunk borders), camera-relative under the world's pose. */
	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$lightOverlay(PoseStack pose, Frustum frustum, MultiBufferSource.BufferSource source, double x, double y, double z,
			boolean translucent, CallbackInfo ci) {
		if (!translucent) {
			com.arcticlauncher.mod.LightOverlayDraw.draw(pose, source, x, y, z);
		}
	}
	//#elif MC >= 1.21.2
	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$lightOverlay(PoseStack pose, Frustum frustum, MultiBufferSource.BufferSource source, double x, double y, double z,
			CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.draw(pose, source, x, y, z);
	}
	//#elif MC >= 1.15
	/** 1.15 - 1.21.1: with the debug renderers (chunk borders); the camera's rotation is set up, positions camera-relative. */
	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$lightOverlay(PoseStack pose, MultiBufferSource.BufferSource source, double x, double y, double z, CallbackInfo ci) {
		//#if MC >= 1.19.4
		com.arcticlauncher.mod.LightOverlayDraw.draw(pose, source, x, y, z);
		//#elif MC >= 1.17
		com.arcticlauncher.mod.LightOverlayDraw.drawShader(x, y, z);
		//#else
		com.arcticlauncher.mod.LightOverlayDraw.drawGl(x, y, z);
		//#endif
	}
	//#else
	/** 1.13 - 1.14: in the world just before the weather (the debug renderers only run when one is on there). */
	@Inject(method = "renderSnowAndRain", at = @At("HEAD"))
	private void arctic$lightOverlay(float partial, CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.drawBeforeWeather(partial);
	}
	//#endif
}
