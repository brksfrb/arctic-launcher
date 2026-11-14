package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.BlockEntityCulling;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Where the world's block entities are drawn (block entity culling only happens in there): before 1.15
 * the entity pass, which is given the frustum; from 1.15 the whole world, whose frustum is taken from
 * the entity test (BlockEntityFrustumMixin).
 */
//#if MC >= 26.2
@Mixin(net.minecraft.client.renderer.extract.LevelExtractor.class)
//#else
@Mixin(net.minecraft.client.renderer.LevelRenderer.class)
//#endif
abstract class BlockEntityCullLevelMixin {
	// (One branch per version, with the name written out, so mixcheck checks it.)
	//#if MC >= 26.2
	@Inject(method = "extract", at = @At("HEAD"))
	private void arctic$beginBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.begin();
	}

	@Inject(method = "extract", at = @At("RETURN"))
	private void arctic$endBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.end();
	}
	//#elif MC >= 26.1
	@Inject(method = "extractLevel", at = @At("HEAD"))
	private void arctic$beginBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.begin();
	}

	@Inject(method = "extractLevel", at = @At("RETURN"))
	private void arctic$endBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.end();
	}
	//#elif MC >= 1.15
	@Inject(method = "renderLevel", at = @At("HEAD"))
	private void arctic$beginBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.begin();
	}

	@Inject(method = "renderLevel", at = @At("RETURN"))
	private void arctic$endBlockEntities(CallbackInfo ci) {
		BlockEntityCulling.end();
	}
	//#elif MC >= 1.14
	@Inject(method = "renderEntities", at = @At("HEAD"))
	private void arctic$beginBlockEntities(net.minecraft.client.Camera camera, net.minecraft.client.renderer.culling.Culler frustum, float partialTick,
			CallbackInfo ci) {
		com.arcticlauncher.mod.BlockEntityCull.beginWorld(frustum);
	}

	@Inject(method = "renderEntities", at = @At("RETURN"))
	private void arctic$endBlockEntities(net.minecraft.client.Camera camera, net.minecraft.client.renderer.culling.Culler frustum, float partialTick,
			CallbackInfo ci) {
		com.arcticlauncher.mod.BlockEntityCull.endWorld();
	}
	//#else
	@Inject(method = "renderEntities", at = @At("HEAD"))
	private void arctic$beginBlockEntities(net.minecraft.world.entity.Entity camera, net.minecraft.client.renderer.culling.Culler frustum, float partialTick,
			CallbackInfo ci) {
		com.arcticlauncher.mod.BlockEntityCull.beginWorld(frustum);
	}

	@Inject(method = "renderEntities", at = @At("RETURN"))
	private void arctic$endBlockEntities(net.minecraft.world.entity.Entity camera, net.minecraft.client.renderer.culling.Culler frustum, float partialTick,
			CallbackInfo ci) {
		com.arcticlauncher.mod.BlockEntityCull.endWorld();
	}
	//#endif
}
