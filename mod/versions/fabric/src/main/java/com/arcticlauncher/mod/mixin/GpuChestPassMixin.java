package com.arcticlauncher.mod.mixin;

//#if MC == 26.2
import com.arcticlauncher.mod.GpuChests;
import net.minecraft.client.renderer.LevelRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The world's main pass is set up: the chests extracted for this frame are drawn in it (and in no other pass that draws solid features). */
@Mixin(LevelRenderer.class)
abstract class GpuChestPassMixin {
	@Inject(method = "addMainPass", at = @At("HEAD"))
	private void arctic$chestsInMainPass(CallbackInfo ci) {
		GpuChests.mainPass();
	}
}
//#endif
