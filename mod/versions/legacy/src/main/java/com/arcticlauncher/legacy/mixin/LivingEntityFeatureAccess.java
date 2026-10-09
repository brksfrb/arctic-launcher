package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** Adding a layer to a living entity's renderer (the method lives here, not on the player renderer). */
@Mixin(LivingEntityRenderer.class)
public interface LivingEntityFeatureAccess {
	@Invoker("addFeature")
	@SuppressWarnings("rawtypes")
	boolean arctic$addFeature(FeatureRenderer feature);
}
