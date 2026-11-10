package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6
import com.arcticlauncher.mod.TextProjection;
import com.mojang.blaze3d.ProjectionType;
import com.mojang.blaze3d.systems.RenderSystem;
//#if MC >= 26.3
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
//#else
import com.mojang.blaze3d.buffers.GpuBufferSlice;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(RenderSystem.class)
abstract class TextProjectionMixin {
	@Inject(method = "setProjectionMatrix", at = @At("HEAD"))
	private static void arctic$projection(GpuBufferSlice slice, ProjectionType type, CallbackInfo ci) {
		TextProjection.select(slice);
	}

	@Inject(method = "backupProjectionMatrix", at = @At("HEAD"))
	private static void arctic$backup(CallbackInfo ci) {
		TextProjection.backup();
	}

	@Inject(method = "restoreProjectionMatrix", at = @At("HEAD"))
	private static void arctic$restore(CallbackInfo ci) {
		TextProjection.restore();
	}
}
//#endif
