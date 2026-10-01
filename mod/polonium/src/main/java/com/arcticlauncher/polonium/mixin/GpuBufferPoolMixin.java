package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.StagingPool;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.GpuDevice;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;

/** Lets Polonium take a staging buffer the same way the game does (a private class). */
@Mixin(targets = "net.minecraft.client.renderer.StagedVertexBuffer$GpuBufferPool")
abstract class GpuBufferPoolMixin implements StagingPool {
	@Shadow
	public abstract GpuBuffer acquire(GpuDevice device, int minSize);

	@Override
	public GpuBuffer polonium$acquire(GpuDevice device, int minSize) {
		return acquire(device, minSize);
	}
}
