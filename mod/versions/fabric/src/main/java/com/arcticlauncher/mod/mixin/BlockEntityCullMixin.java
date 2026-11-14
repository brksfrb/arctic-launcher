package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.BlockEntityCull;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher;
import net.minecraft.world.level.block.entity.BlockEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;

/**
 * Chests, signs and the like outside the view are not drawn (the game draws every one in a section
 * that is partly on screen). Before the renderer is looked up and its state made, so a skipped one
 * costs nothing more; what is drawn is drawn exactly as before.
 */
@Mixin(BlockEntityRenderDispatcher.class)
abstract class BlockEntityCullMixin {
	//#if MC >= 26.2
	@Inject(method = "tryExtractRenderState", at = @At("HEAD"), cancellable = true)
	private void arctic$cull(BlockEntity entity, float partialTick, net.minecraft.client.renderer.feature.ModelFeatureRenderer.CrumblingOverlay crumbling,
			boolean global, org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Object> cir) {
		//#if MC == 26.2
		// A chest drawn on the GPU needs no render state (GpuChests): shut ones are already in their section's instances.
		if (com.arcticlauncher.mod.GpuChests.capture(entity, partialTick, crumbling, global)) {
			cir.setReturnValue(null);
			return;
		}
		//#endif
		if (BlockEntityCull.skip(entity)) {
			cir.setReturnValue(null);
		}
	}

	//#if MC == 26.2
	@Inject(method = "onResourceManagerReload", at = @At("RETURN"))
	private void arctic$reloadChests(org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
		com.arcticlauncher.mod.GpuChests.reload();
	}
	//#endif
	//#elif MC >= 1.21.9
	@Inject(method = "tryExtractRenderState", at = @At("HEAD"), cancellable = true)
	private void arctic$cull(BlockEntity entity, float partialTick, net.minecraft.client.renderer.feature.ModelFeatureRenderer.CrumblingOverlay crumbling,
			org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Object> cir) {
		if (BlockEntityCull.skip(entity)) {
			cir.setReturnValue(null);
		}
	}
	//#elif MC >= 1.15
	@Inject(method = "render(Lnet/minecraft/world/level/block/entity/BlockEntity;FLcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;)V",
			at = @At("HEAD"), cancellable = true)
	private void arctic$cull(BlockEntity entity, float partialTick, com.mojang.blaze3d.vertex.PoseStack pose, net.minecraft.client.renderer.MultiBufferSource buffers,
			org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
		if (BlockEntityCull.skip(entity)) {
			ci.cancel();
		}
	}
	//#else
	@Inject(method = "render(Lnet/minecraft/world/level/block/entity/BlockEntity;FI)V", at = @At("HEAD"), cancellable = true)
	private void arctic$cull(BlockEntity entity, float partialTick, int destroyStage, org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
		if (BlockEntityCull.skip(entity)) {
			ci.cancel();
		}
	}
	//#endif
}
