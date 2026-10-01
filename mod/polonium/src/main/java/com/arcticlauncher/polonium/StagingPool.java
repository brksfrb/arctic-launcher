package com.arcticlauncher.polonium;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.GpuDevice;

/** The game's staging buffer pool (a private class), as Polonium sees it. */
public interface StagingPool {
	GpuBuffer polonium$acquire(GpuDevice device, int minSize);
}
