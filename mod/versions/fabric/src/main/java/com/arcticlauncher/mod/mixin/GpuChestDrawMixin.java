package com.arcticlauncher.mod.mixin;

//#if MC == 26.2
import com.arcticlauncher.mod.GpuChests;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The frame's chests are drawn where the game draws its solid entities and block entities: after the solid terrain, before anything see-through. */
@Mixin(targets = "net.minecraft.client.renderer.feature.FeatureRenderDispatcher$PreparedFrame")
abstract class GpuChestDrawMixin {
	@Inject(method = "executeSolid", at = @At("HEAD"))
	private void arctic$drawChests(CallbackInfo ci) {
		GpuChests.draw();
	}
}
//#endif
