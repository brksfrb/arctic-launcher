//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelUpload;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.GpuDevice;
import java.util.List;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Each frame the game copies every entity's vertices into one GPU buffer and
 * sorts the see-through ones, all on the render thread. With lots on screen
 * Polonium does the same copies and sorts on several threads; the bytes and
 * their order are exactly the game's. Small frames keep the game's code.
 */
@Mixin(StagedVertexBuffer.class)
abstract class StagedVertexBufferMixin {
	@Inject(method = "uploadDrawsToBuffers", at = @At("HEAD"), cancellable = true)
	private void polonium$parallelUpload(GpuDevice device, List<StagedVertexBuffer.Draw> draws, GpuBuffer vertexBuffer,
			@Nullable GpuBuffer indexBuffer, int vertexBufferSize, int indexBufferSize, CallbackInfo ci) {
		if (ParallelUpload.upload((StagedVertexBuffer) (Object) this, device, draws, vertexBuffer, indexBuffer,
				vertexBufferSize, indexBufferSize)) {
			ci.cancel();
		}
	}
}
//#endif
