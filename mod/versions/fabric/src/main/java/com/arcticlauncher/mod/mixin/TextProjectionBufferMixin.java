package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6
import com.arcticlauncher.mod.TextProjection;
//#if MC >= 26.3
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
//#else
import com.mojang.blaze3d.buffers.GpuBufferSlice;
//#endif
//#if MC >= 26.1
import net.minecraft.client.renderer.Projection;
import net.minecraft.client.renderer.ProjectionMatrixBuffer;
//#else
import net.minecraft.client.renderer.PerspectiveProjectionMatrixBuffer;
//#endif
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

//#if MC >= 26.1
@Mixin(ProjectionMatrixBuffer.class)
//#else
@Mixin(PerspectiveProjectionMatrixBuffer.class)
//#endif
abstract class TextProjectionBufferMixin {
	@Unique private final Matrix4f arctic$matrix = new Matrix4f();
	@Unique private GpuBufferSlice arctic$slice;

	//#if MC >= 26.3
	@Inject(method = "getBuffer(Lorg/joml/Matrix4f;)Lcom/mojang/renderpearl/api/buffers/GpuBufferSlice;", at = @At("RETURN"))
	//#else
	@Inject(method = "getBuffer(Lorg/joml/Matrix4f;)Lcom/mojang/blaze3d/buffers/GpuBufferSlice;", at = @At("RETURN"))
	//#endif
	private void arctic$remember(Matrix4f matrix, CallbackInfoReturnable<GpuBufferSlice> cir) {
		arctic$matrix.set(matrix);
		arctic$slice = cir.getReturnValue();
		TextProjection.remember(arctic$slice, arctic$matrix);
	}

	//#if MC >= 26.1
	//#if MC >= 26.3
	@Inject(method = "getBuffer(Lnet/minecraft/client/renderer/Projection;)Lcom/mojang/renderpearl/api/buffers/GpuBufferSlice;", at = @At("RETURN"))
	//#else
	@Inject(method = "getBuffer(Lnet/minecraft/client/renderer/Projection;)Lcom/mojang/blaze3d/buffers/GpuBufferSlice;", at = @At("RETURN"))
	//#endif
	private void arctic$rememberProjection(Projection projection, CallbackInfoReturnable<GpuBufferSlice> cir) {
		projection.getMatrix(arctic$matrix);
		arctic$slice = cir.getReturnValue();
		TextProjection.remember(arctic$slice, arctic$matrix);
	}
	//#endif

	@Inject(method = "close", at = @At("HEAD"))
	private void arctic$forget(CallbackInfo ci) {
		TextProjection.forget(arctic$slice);
	}
}
//#endif
