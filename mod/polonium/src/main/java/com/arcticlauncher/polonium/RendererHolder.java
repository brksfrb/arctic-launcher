package com.arcticlauncher.polonium;

import net.minecraft.client.renderer.entity.EntityRenderer;

/** Room on each entity for its renderer and the tick it was found in (added to Entity by a mixin). */
public interface RendererHolder {
	EntityRenderer<?, ?> polonium$renderer();

	int polonium$rendererTick();

	void polonium$renderer(EntityRenderer<?, ?> renderer, int tick);
}
