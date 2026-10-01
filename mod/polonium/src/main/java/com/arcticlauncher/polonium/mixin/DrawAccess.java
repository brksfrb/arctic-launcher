package com.arcticlauncher.polonium.mixin;

import com.mojang.blaze3d.IndexType;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.VertexSorting;
import java.util.List;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(StagedVertexBuffer.Draw.class)
public interface DrawAccess {
	@Accessor("vertexOffset")
	int polonium$vertexOffset();

	@Accessor("indexOffset")
	int polonium$indexOffset();

	@Accessor("quadSorting")
	VertexSorting polonium$quadSorting();

	@Accessor("vertexBufferSlices")
	List<ByteBufferBuilder.Result> polonium$slices();

	@Invoker("indexType")
	IndexType polonium$indexType();

	@Invoker("freeVertexData")
	void polonium$freeVertexData();
}
