package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.11 && MC < 26.1
import net.minecraft.client.renderer.LevelRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Light Overlay: drawn in the world pass right after the block outline (finalizing the gizmos is the next step there). */
@Mixin(LevelRenderer.class)
abstract class LightOverlayMixin {
	@Inject(method = "finalizeGizmoCollection", at = @At("HEAD"), require = 0)
	private void arctic$lightOverlay(CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.draw();
	}
}
//#endif
