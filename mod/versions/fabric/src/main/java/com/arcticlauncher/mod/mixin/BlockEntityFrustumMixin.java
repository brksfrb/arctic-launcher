package com.arcticlauncher.mod.mixin;

//#if MC >= 1.15
import com.arcticlauncher.client.feature.BlockEntityCulling;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The frame's view frustum, for block entity culling: the game tests every entity against it before
 * it draws the block entities, which aren't given one (1.15+).
 */
@Mixin(EntityRenderDispatcher.class)
abstract class BlockEntityFrustumMixin {
	//#if MC >= 26.3
	@Inject(method = "shouldRender", at = @At("HEAD"))
	private void arctic$frustum(Entity entity, Frustum frustum, double camX, double camY, double camZ, float partialTick,
			CallbackInfoReturnable<Boolean> cir) {
		BlockEntityCulling.seen(frustum);
	}
	//#else
	@Inject(method = "shouldRender", at = @At("HEAD"))
	private void arctic$frustum(Entity entity, Frustum frustum, double camX, double camY, double camZ, CallbackInfoReturnable<Boolean> cir) {
		BlockEntityCulling.seen(frustum);
	}
	//#endif
}
//#endif
