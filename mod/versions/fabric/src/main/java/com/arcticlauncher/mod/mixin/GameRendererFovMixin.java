package com.arcticlauncher.mod.mixin;

//#if MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Camera;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Zoom, before 26.1: FOV is computed on GameRenderer, not Camera. */
@Mixin(GameRenderer.class)
abstract class GameRendererFovMixin {
	@Inject(method = "getFov", at = @At("RETURN"), cancellable = true)
	//#if MC >= 1.21.2
	private void arctic$zoom(Camera camera, float partial, boolean useFov, CallbackInfoReturnable<Float> cir) {
		float replay = com.arcticlauncher.mod.replay.ReplayView.fov;
		float fov = replay > 0 && useFov ? replay : cir.getReturnValue();
		cir.setReturnValue(fov * ArcticClient.features().fovMultiplier());
		if (useFov) {
			com.arcticlauncher.mod.Compat.worldFov = cir.getReturnValue();
		}
	}
	//#else
	// 1.20.1 - 1.21.1: getFov still returned a double.
	private void arctic$zoom(Camera camera, float partial, boolean useFov, CallbackInfoReturnable<Double> cir) {
		float replay = com.arcticlauncher.mod.replay.ReplayView.fov;
		double fov = replay > 0 && useFov ? replay : cir.getReturnValue();
		cir.setReturnValue(fov * ArcticClient.features().fovMultiplier());
		if (useFov) {
			com.arcticlauncher.mod.Compat.worldFov = cir.getReturnValue();
		}
	}
	//#endif
}
//#endif
