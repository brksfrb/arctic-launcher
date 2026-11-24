//#if MC >= 1.21.5
package com.arcticlauncher.mod.replay;

import com.arcticlauncher.client.replay.LiveVideo;
import com.arcticlauncher.client.replay.ReplayBackend;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.pipeline.TextureTarget;
import com.mojang.blaze3d.systems.RenderSystem;
//#if MC >= 26.3
import com.mojang.renderpearl.api.GpuFormat;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.textures.GpuTexture;
//#else
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.textures.GpuTexture;
//#if MC >= 26.2
import com.mojang.blaze3d.GpuFormat;
//#endif
//#endif

/** A persistent downscaled render target and two asynchronous readback slots. */
final class GpuLiveFrames {
	private static final int READ_BACK = 9;
	private static final Slot[] SLOTS = {new Slot(), new Slot()};
	private static TextureTarget target;

	private static final class Slot {
		GpuBuffer buffer;
		boolean pending;
	}

	private GpuLiveFrames() {}

	static void capture(int height, ReplayBackend.FrameSink sink) {
		RenderTarget source = ReplayCompat.mainTarget();
		if (source.getColorTexture() == null) return;
		int[] size = LiveVideo.size(source.width, source.height, height);
		Slot slot = null;
		for (Slot candidate : SLOTS) {
			if (!candidate.pending) { slot = candidate; break; }
		}
		if (slot == null) return;
		if (target == null || target.width != size[0] || target.height != size[1]) {
			for (Slot candidate : SLOTS) if (candidate.pending) return;
			resize(size[0], size[1]);
		}
		//#if MC >= 26.2
		source.blitAndBlendToTexture(target.getColorTextureView(), null);
		//#elif MC >= 1.21.6
		source.blitAndBlendToTexture(target.getColorTextureView());
		//#else
		source.blitAndBlendToTexture(target.getColorTexture());
		//#endif
		read(slot, target.getColorTexture(), size[0], size[1], sink);
	}

	private static void resize(int width, int height) {
		if (target != null) target.destroyBuffers();
		for (Slot slot : SLOTS) {
			if (slot.buffer != null) slot.buffer.close();
			slot.buffer = null;
		}
		//#if MC >= 26.3
		target = new TextureTarget("Arctic gameplay capture", width, height, GpuFormat.RGBA8_UNORM, null);
		//#elif MC >= 26.2
		target = new TextureTarget("Arctic gameplay capture", width, height, false, GpuFormat.RGBA8_UNORM);
		//#else
		target = new TextureTarget("Arctic gameplay capture", width, height, false);
		//#endif
	}

	private static void read(Slot slot, GpuTexture texture, int width, int height, ReplayBackend.FrameSink sink) {
		int bytes = Math.multiplyExact(Math.multiplyExact(width, height), 4);
		if (slot.buffer == null) {
			//#if MC >= 1.21.6
			slot.buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic gameplay readback", READ_BACK, bytes);
			//#else
			slot.buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic gameplay readback",
					com.mojang.blaze3d.buffers.BufferType.PIXEL_PACK, com.mojang.blaze3d.buffers.BufferUsage.STATIC_READ, bytes);
			//#endif
		}
		var encoder = RenderSystem.getDevice().createCommandEncoder();
		slot.pending = true;
		try {
			encoder.copyTextureToBuffer(texture, slot.buffer, 0, () -> {
				try {
					//#if MC >= 26.2
					try (var view = slot.buffer.map(true, false)) {
					//#elif MC >= 1.21.6
					try (var view = encoder.mapBuffer(slot.buffer, true, false)) {
					//#else
					try (var view = encoder.readBuffer(slot.buffer)) {
					//#endif
						sink.frame(width, height, view.data(), true);
					}
				} finally {
					slot.pending = false;
				}
			}, 0);
		} catch (RuntimeException e) {
			slot.pending = false;
			throw e;
		}
	}
}
//#endif
