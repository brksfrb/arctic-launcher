package com.arcticlauncher.polonium.mixin;

import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The renderer's name tag extraction with the distances given. */
@Mixin(EntityRenderer.class)
public interface EntityRendererNameTagsAccess {
	@Invoker("extractNameTags")
	void polonium$extractNameTags(Entity entity, EntityRenderState state, float partialTicks, double nameTagDistance, double belowNameDistance);
}
