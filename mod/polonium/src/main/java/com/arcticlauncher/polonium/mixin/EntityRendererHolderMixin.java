package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.RendererHolder;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on each entity for its kept renderer (see EntityRendererCacheMixin). */
@Mixin(Entity.class)
abstract class EntityRendererHolderMixin implements RendererHolder {
	@Unique
	private EntityRenderer<?, ?> polonium$renderer;

	@Unique
	private int polonium$rendererTick = -1;

	@Override
	public EntityRenderer<?, ?> polonium$renderer() {
		return polonium$renderer;
	}

	@Override
	public int polonium$rendererTick() {
		return polonium$rendererTick;
	}

	@Override
	public void polonium$renderer(EntityRenderer<?, ?> renderer, int tick) {
		polonium$renderer = renderer;
		polonium$rendererTick = tick;
	}
}
