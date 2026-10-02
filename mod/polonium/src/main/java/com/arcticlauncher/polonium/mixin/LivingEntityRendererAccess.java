package com.arcticlauncher.polonium.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import java.util.List;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

/** What the crowd path needs to place an entity's model the way {@link LivingEntityRenderer#submit} does. */
@Mixin(LivingEntityRenderer.class)
public interface LivingEntityRendererAccess {
	@Invoker("setupRotations")
	void polonium$setupRotations(LivingEntityRenderState state, PoseStack poseStack, float bodyRot, float entityScale);

	@Invoker("scale")
	void polonium$scale(LivingEntityRenderState state, PoseStack poseStack);

	@Accessor("layers")
	List<RenderLayer<?, ?>> polonium$layers();
}
