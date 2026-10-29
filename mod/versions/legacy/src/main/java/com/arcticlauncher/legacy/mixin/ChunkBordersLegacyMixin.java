package com.arcticlauncher.legacy.mixin;

//#if MC < 1.10
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Chunk Borders on 1.8.9 and 1.9.4: the lines are drawn in the world, just before the weather. */
@Mixin(GameRenderer.class)
abstract class ChunkBordersLegacyMixin {
	@Inject(method = "renderWorld(IFJ)V", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/GameRenderer;renderWeather(F)V"), require = 0)
	private void arctic$chunkBorders(int pass, float tickDelta, long nanoTime, CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyChunkBorders.draw(tickDelta);
		//#if MC < 1.9
		com.arcticlauncher.legacy.LegacyTeamBoxes.draw(tickDelta);
		//#endif
	}
}
//#endif
