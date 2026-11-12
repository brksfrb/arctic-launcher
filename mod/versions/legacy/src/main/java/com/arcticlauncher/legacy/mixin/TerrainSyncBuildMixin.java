package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.TerrainWork;
import net.minecraft.client.render.chunk.ChunkBuilder;
import net.minecraft.client.world.BuiltChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The camera's own section is built on the client thread, inside the frame. When its last build was slow it is
 * queued for the meshing threads like every other section (the old mesh stays on screen until the new one is ready).
 */
@Mixin(ChunkBuilder.class)
abstract class TerrainSyncBuildMixin {
	@Shadow
	public abstract boolean send(BuiltChunk chunk);

	@Inject(method = "upload(Lnet/minecraft/client/world/BuiltChunk;)Z", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$heavyLater(BuiltChunk chunk, CallbackInfoReturnable<Boolean> cir) {
		if (((TerrainWork.Timed) (Object) chunk).arctic$buildNanos() > TerrainWork.SLOW_BUILD_NANOS && send(chunk)) {
			cir.setReturnValue(true);
		}
	}
}
