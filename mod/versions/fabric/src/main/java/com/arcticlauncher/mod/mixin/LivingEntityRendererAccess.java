package com.arcticlauncher.mod.mixin;

import net.minecraft.client.renderer.entity.LivingEntityRenderer;
//#if MC >= 1.14
import net.minecraft.client.renderer.entity.layers.RenderLayer;
//#else
import net.minecraft.client.render.entity.feature.FeatureRenderer;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** Lets Arctic add its render layers to living-entity renderers. */
@Mixin(LivingEntityRenderer.class)
public interface LivingEntityRendererAccess {
	@SuppressWarnings({"rawtypes", "unchecked"})
	//#if MC >= 1.14
	@Invoker("addLayer")
	boolean arctic$addLayer(RenderLayer layer);
	//#else
	// 1.13 calls them features.
	@Invoker("addFeature")
	boolean arctic$addLayer(FeatureRenderer layer);
	//#endif
}
