package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyBlockEntities;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.CameraView;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The frustum the world's entities are drawn with, for the block entities drawn in the same pass
 * (see LegacyBlockEntities). Outside this pass (items, menus) nothing is culled.
 */
@Mixin(WorldRenderer.class)
abstract class BlockEntityCullWorldMixin {
	@Inject(method = "renderEntities", at = @At("HEAD"))
	private void arctic$beginBlockEntities(Entity camera, CameraView frustum, float tickDelta, CallbackInfo ci) {
		LegacyBlockEntities.beginWorld(frustum, MinecraftClient.getInstance().textRenderer);
	}

	@Inject(method = "renderEntities", at = @At("RETURN"))
	private void arctic$endBlockEntities(Entity camera, CameraView frustum, float tickDelta, CallbackInfo ci) {
		LegacyBlockEntities.endWorld();
	}
}
