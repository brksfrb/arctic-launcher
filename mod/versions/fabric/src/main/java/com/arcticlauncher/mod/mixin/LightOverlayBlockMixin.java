package com.arcticlauncher.mod.mixin;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;
//#if MC >= 26.2
import net.minecraft.client.multiplayer.ClientLevel;
//#else
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.world.level.BlockGetter;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Light Overlay: a block changing near you (a torch placed or broken) is rescanned at once, not on the next round. */
//#if MC >= 26.2
@Mixin(ClientLevel.class)
//#else
@Mixin(LevelRenderer.class)
//#endif
abstract class LightOverlayBlockMixin {
	//#if MC >= 26.2
	@Inject(method = "sendBlockUpdated", at = @At("HEAD"))
	private void arctic$lightOverlayBlock(BlockPos pos, BlockState old, BlockState now, int flags, CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.blockChanged(pos);
	}
	//#else
	//#if MC >= 1.14
	@Inject(method = "blockChanged", at = @At("HEAD"))
	//#else
	@Inject(method = "method_11493", at = @At("HEAD"))
	//#endif
	private void arctic$lightOverlayBlock(BlockGetter level, BlockPos pos, BlockState old, BlockState now, int flags, CallbackInfo ci) {
		com.arcticlauncher.mod.LightOverlayDraw.blockChanged(pos);
	}
	//#endif
}
