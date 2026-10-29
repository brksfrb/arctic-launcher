package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.16
import com.mojang.blaze3d.vertex.PoseStack;
//#endif
//#if MC >= 26.1
import net.minecraft.client.renderer.state.level.CameraRenderState;
//#endif

/** No Hurt Shake: the view doesn't tilt and shake when you take damage. */
@Mixin(GameRenderer.class)
abstract class NoHurtCamMixin {
	//#if MC >= 26.1
	@Inject(method = "bobHurt", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$noShake(CameraRenderState camera, PoseStack pose, CallbackInfo ci) {
	//#elif MC >= 1.16
	@Inject(method = "bobHurt", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$noShake(PoseStack pose, float partialTicks, CallbackInfo ci) {
	//#else
	@Inject(method = "bobHurt", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$noShake(float partialTicks, CallbackInfo ci) {
	//#endif
		if (ArcticClient.config() != null && ArcticClient.config().noHurtCam) {
			ci.cancel();
		}
	}
}
//#endif
