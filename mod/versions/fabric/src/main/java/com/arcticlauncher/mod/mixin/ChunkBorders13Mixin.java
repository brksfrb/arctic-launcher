package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13 && MC < 1.14
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Chunk Borders on 1.13: the lines are drawn in the world, just before the weather. */
@Mixin(GameRenderer.class)
abstract class ChunkBorders13Mixin {
	@Inject(method = "renderSnowAndRain", at = @At("HEAD"))
	private void arctic$chunkBorders(float partial, CallbackInfo ci) {
		com.arcticlauncher.mod.ChunkBorders13.draw(partial);
	}
}
//#endif
