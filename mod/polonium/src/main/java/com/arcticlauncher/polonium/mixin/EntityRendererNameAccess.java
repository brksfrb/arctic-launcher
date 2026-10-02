package com.arcticlauncher.polonium.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The renderer's name tag submission (players add their score line). */
@Mixin(EntityRenderer.class)
public interface EntityRendererNameAccess {
	@Invoker("submitNameDisplay")
	void polonium$submitNameDisplay(EntityRenderState state, PoseStack poseStack, SubmitNodeCollector collector, CameraRenderState camera);
}
