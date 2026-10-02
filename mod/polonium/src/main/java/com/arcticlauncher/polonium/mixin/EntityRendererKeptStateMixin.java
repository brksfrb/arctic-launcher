package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.KeptStates;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Other players' render states kept from frame to frame (see {@link KeptStates}); the game fills them in as always. */
@Mixin(EntityRenderer.class)
abstract class EntityRendererKeptStateMixin {
	@WrapOperation(method = "createRenderState(Lnet/minecraft/world/entity/Entity;F)Lnet/minecraft/client/renderer/entity/state/EntityRenderState;",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/entity/EntityRenderer;createRenderState()Lnet/minecraft/client/renderer/entity/state/EntityRenderState;"))
	private EntityRenderState polonium$kept(EntityRenderer<?, ?> self, Operation<EntityRenderState> create, Entity entity, float partialTicks) {
		if (!KeptStates.inLevel() || !(entity instanceof Avatar) || entity instanceof LocalPlayer
				|| !(entity instanceof KeptStates.Holder holder)) {
			return create.call(self);
		}
		EntityRenderState kept = holder.polonium$keptState();
		if (kept != null && holder.polonium$keptStateRenderer() == self) {
			return kept;
		}
		EntityRenderState made = create.call(self);
		holder.polonium$keptState(made, self);
		return made;
	}
}
