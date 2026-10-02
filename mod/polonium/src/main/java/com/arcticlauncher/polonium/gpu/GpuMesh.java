//#if MC >= 26.2
package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.buffers.GpuBuffer;
import net.minecraft.resources.Identifier;

/** A shape kept on the GPU, drawn once per entity from per-entity instance data. */
abstract class GpuMesh {
	final GpuBuffer vertices;
	final int vertexCount;
	/** Texels each entity takes in the instance buffer (the same for every entity of this mesh). */
	final int texelsPerInstance;
	/** The vertex shader that reads this mesh and its instance data. */
	final Identifier vertexShader;
	long lastUsedFrame;

	GpuMesh(GpuBuffer vertices, int vertexCount, int texelsPerInstance, Identifier vertexShader) {
		this.vertices = vertices;
		this.vertexCount = vertexCount;
		this.texelsPerInstance = texelsPerInstance;
		this.vertexShader = vertexShader;
	}

	void close() {
		vertices.close();
	}
}
//#endif
