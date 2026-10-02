package com.arcticlauncher.polonium.mixin;

import it.unimi.dsi.fastutil.ints.IntList;
import java.util.List;
import net.minecraft.client.resources.model.cuboid.ItemTransform;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.special.SpecialModelRenderer;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** One layer of an item's render state (see {@link ItemStackRenderStateAccess}). */
@Mixin(ItemStackRenderState.LayerRenderState.class)
public interface ItemLayerRenderStateAccess {
	@Accessor("quads")
	List<BakedQuad> polonium$quads();

	@Accessor("itemTransform")
	ItemTransform polonium$itemTransform();

	@Accessor("localTransform")
	Matrix4f polonium$localTransform();

	@Accessor("foilType")
	ItemStackRenderState.FoilType polonium$foilType();

	@Accessor("tintLayers")
	@Nullable IntList polonium$tintLayers();

	@Accessor("specialRenderer")
	@Nullable SpecialModelRenderer<Object> polonium$specialRenderer();
}
