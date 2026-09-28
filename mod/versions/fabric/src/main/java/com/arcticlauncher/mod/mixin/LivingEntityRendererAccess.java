package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** Lets Arctic add its render layers to living-entity renderers. */
@Mixin(LivingEntityRenderer.class)
public interface LivingEntityRendererAccess {
	@SuppressWarnings({"rawtypes", "unchecked"})
	@Invoker("addLayer")
	boolean arctic$addLayer(RenderLayer layer);
}
//#endif
