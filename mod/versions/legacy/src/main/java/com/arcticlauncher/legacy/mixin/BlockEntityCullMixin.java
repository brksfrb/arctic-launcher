package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyBlockEntities;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.client.render.block.entity.BlockEntityRenderDispatcher;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Chests, signs and the like outside the view are not drawn (the game draws every one in a section
 * that is partly on screen). Before the game's distance check and light lookup, so a skipped one costs
 * nothing more.
 */
@Mixin(BlockEntityRenderDispatcher.class)
abstract class BlockEntityCullMixin {
	@Shadow
	public double cameraX;
	@Shadow
	public double cameraY;
	@Shadow
	public double cameraZ;

	@Inject(method = "renderEntity(Lnet/minecraft/block/entity/BlockEntity;FI)V", at = @At("HEAD"), cancellable = true)
	private void arctic$cull(BlockEntity entity, float tickDelta, int destroyStage, CallbackInfo ci) {
		if (LegacyBlockEntities.skip(entity, cameraX, cameraY, cameraZ)) {
			ci.cancel();
		}
	}
}
