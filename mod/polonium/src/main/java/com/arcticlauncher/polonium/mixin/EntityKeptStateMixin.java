package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.KeptStates;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on an entity for its kept render state (see {@link KeptStates}). */
@Mixin(Entity.class)
abstract class EntityKeptStateMixin implements KeptStates.Holder {
	@Unique
	private EntityRenderState polonium$keptState;
	@Unique
	private EntityRenderer<?, ?> polonium$keptStateRenderer;

	@Override
	public EntityRenderState polonium$keptState() {
		return polonium$keptState;
	}

	@Override
	public EntityRenderer<?, ?> polonium$keptStateRenderer() {
		return polonium$keptStateRenderer;
	}

	@Override
	public void polonium$keptState(EntityRenderState state, EntityRenderer<?, ?> renderer) {
		polonium$keptState = state;
		polonium$keptStateRenderer = renderer;
	}
}
