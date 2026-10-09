package com.arcticlauncher.mod.mixin;

//#if MC < 26.1
import com.arcticlauncher.mod.MotionBlurGL;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Motion blur before 26.1: right after the world is drawn, before the HUD (see {@link MotionBlurGL}). */
@Mixin(GameRenderer.class)
abstract class MotionBlurOldMixin {
	//#if MC >= 1.21
	@Inject(method = "render(Lnet/minecraft/client/DeltaTracker;Z)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/GameRenderer;renderLevel(Lnet/minecraft/client/DeltaTracker;)V", shift = At.Shift.AFTER))
	//#elif MC >= 1.20.5
	@Inject(method = "render(FJZ)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/GameRenderer;renderLevel(FJ)V", shift = At.Shift.AFTER))
	//#else
	@Inject(method = "render(FJZ)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/GameRenderer;renderLevel(FJLcom/mojang/blaze3d/vertex/PoseStack;)V", shift = At.Shift.AFTER))
	//#endif
	private void arctic$motionBlur(CallbackInfo ci) {
		MotionBlurGL.apply();
	}
}
//#endif
