package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.render.WorldRenderer;
import net.minecraft.util.math.BlockPos;
//#if MC >= 1.9
import net.minecraft.block.BlockState;
import net.minecraft.world.World;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Light Overlay: a block changing near you (a torch placed or broken) is rescanned at once, not on the next round. */
@Mixin(WorldRenderer.class)
abstract class LightOverlayBlockLegacyMixin {
	//#if MC >= 1.9
	@Inject(method = "method_11493", at = @At("HEAD"))
	private void arctic$lightOverlayBlock(World world, BlockPos pos, BlockState old, BlockState now, int flags, CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyLightOverlay.blockChanged(pos);
	}
	//#else
	@Inject(method = "onBlockUpdate", at = @At("HEAD"))
	private void arctic$lightOverlayBlock(BlockPos pos, CallbackInfo ci) {
		com.arcticlauncher.legacy.LegacyLightOverlay.blockChanged(pos);
	}
	//#endif
}
