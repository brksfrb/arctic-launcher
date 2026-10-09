//#if MC >= 1.15
package com.arcticlauncher.mod.replay;

import com.arcticlauncher.client.replay.ReplayBackend;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;

/**
 * Reads the finished frame back from the GPU for video export and replay
 * thumbnails, handing the pixels over as they are (rows bottom first, the
 * way the GPU keeps them).
 */
final class ReplayFrames {
	/** Buffer usage: the CPU maps it for reading, the GPU copies into it. */
	private static final int READ_BACK = 9;

	private ReplayFrames() {}

	static void capture(final ReplayBackend.FrameSink sink) {
		RenderTarget target = ReplayCompat.mainTarget();
		final int width = target.width;
		final int height = target.height;
		//#if MC >= 26.3
		final com.mojang.renderpearl.api.textures.GpuTexture texture = target.getColorTexture();
		if (texture == null) {
			return;
		}
		final com.mojang.renderpearl.api.buffers.GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic replay frame",
				READ_BACK, (long) width * height * texture.getFormat().blockSize());
		RenderSystem.getDevice().createCommandEncoder().copyTextureToBuffer(texture, buffer, 0L, () -> {
			try (com.mojang.renderpearl.api.buffers.GpuBufferSlice.MappedView read = buffer.map(true, false)) {
				sink.frame(width, height, read.data(), true);
			}
			buffer.close();
		}, 0);
		//#elif MC >= 26.2
		final com.mojang.blaze3d.textures.GpuTexture texture = target.getColorTexture();
		if (texture == null) {
			return;
		}
		final com.mojang.blaze3d.buffers.GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic replay frame",
				READ_BACK, (long) width * height * texture.getFormat().blockSize());
		RenderSystem.getDevice().createCommandEncoder().copyTextureToBuffer(texture, buffer, 0L, () -> {
			try (com.mojang.blaze3d.buffers.GpuBufferSlice.MappedView read = buffer.map(true, false)) {
				sink.frame(width, height, read.data(), true);
			}
			buffer.close();
		}, 0);
		//#elif MC >= 1.21.6
		final com.mojang.blaze3d.textures.GpuTexture texture = target.getColorTexture();
		if (texture == null) {
			return;
		}
		final com.mojang.blaze3d.systems.CommandEncoder encoder = RenderSystem.getDevice().createCommandEncoder();
		//#if MC >= 1.21.11
		final com.mojang.blaze3d.buffers.GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic replay frame",
				READ_BACK, (long) width * height * texture.getFormat().pixelSize());
		encoder.copyTextureToBuffer(texture, buffer, 0L, () -> {
		//#else
		final com.mojang.blaze3d.buffers.GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic replay frame",
				READ_BACK, width * height * texture.getFormat().pixelSize());
		encoder.copyTextureToBuffer(texture, buffer, 0, () -> {
		//#endif
			try (com.mojang.blaze3d.buffers.GpuBuffer.MappedView read = encoder.mapBuffer(buffer, true, false)) {
				sink.frame(width, height, read.data(), true);
			}
			buffer.close();
		}, 0);
		//#elif MC >= 1.21.5
		final com.mojang.blaze3d.textures.GpuTexture texture = target.getColorTexture();
		if (texture == null) {
			return;
		}
		final com.mojang.blaze3d.systems.CommandEncoder encoder = RenderSystem.getDevice().createCommandEncoder();
		final com.mojang.blaze3d.buffers.GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic replay frame",
				com.mojang.blaze3d.buffers.BufferType.PIXEL_PACK, com.mojang.blaze3d.buffers.BufferUsage.STATIC_READ,
				width * height * texture.getFormat().pixelSize());
		encoder.copyTextureToBuffer(texture, buffer, 0, () -> {
			try (com.mojang.blaze3d.buffers.GpuBuffer.ReadView read = encoder.readBuffer(buffer)) {
				sink.frame(width, height, read.data(), true);
			}
			buffer.close();
		}, 0);
		//#else
		// OpenGL directly: read the color texture into memory we own.
		java.nio.ByteBuffer pixels = org.lwjgl.system.MemoryUtil.memAlloc(width * height * 4);
		try {
			//#if MC >= 1.16
			RenderSystem.bindTexture(target.getColorTextureId());
			//#else
			RenderSystem.bindTexture(target.colorTextureId);
			//#endif
			org.lwjgl.opengl.GL11.glPixelStorei(org.lwjgl.opengl.GL11.GL_PACK_ALIGNMENT, 1);
			org.lwjgl.opengl.GL11.glGetTexImage(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, 0, org.lwjgl.opengl.GL11.GL_RGBA,
					org.lwjgl.opengl.GL11.GL_UNSIGNED_BYTE, pixels);
			sink.frame(width, height, pixels, true);
		} finally {
			org.lwjgl.system.MemoryUtil.memFree(pixels);
		}
		//#endif
	}
}
//#endif
