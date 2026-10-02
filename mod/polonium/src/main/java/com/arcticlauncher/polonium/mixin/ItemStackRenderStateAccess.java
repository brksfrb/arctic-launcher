package com.arcticlauncher.polonium.mixin;

import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.item.ItemDisplayContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** An item's render state, to tell whether it still looks the way it did. */
@Mixin(ItemStackRenderState.class)
public interface ItemStackRenderStateAccess {
	@Accessor("activeLayerCount")
	int polonium$layerCount();

	@Accessor("layers")
	ItemStackRenderState.LayerRenderState[] polonium$layers();

	@Accessor("displayContext")
	ItemDisplayContext polonium$displayContext();
}
