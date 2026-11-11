package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.5
import com.arcticlauncher.client.Fog;
//#if MC >= 1.21.6
import net.minecraft.client.renderer.fog.FogRenderer;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#else
import net.minecraft.client.renderer.FogParameters;
import net.minecraft.client.renderer.FogRenderer;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Fog on 1.21.5 and newer, where the fog goes to the GPU as a buffer: 1.21.5 builds a FogParameters in
 * FogRenderer.setupFog (stretched on its way out); 1.21.6 and newer write the distances into the buffer in
 * FogRenderer.updateBuffer (the four distances are stretched on their way in).
 */
@Mixin(FogRenderer.class)
abstract class FogBufferMixin {
	//#if MC >= 1.21.6
	private static final String WRITE = "updateBuffer(Ljava/nio/ByteBuffer;ILorg/joml/Vector4f;FFFFFF)V";

	@ModifyVariable(method = WRITE, at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$environmentStart(float value) {
		return Fog.stretch(value);
	}

	@ModifyVariable(method = WRITE, at = @At("HEAD"), argsOnly = true, ordinal = 1)
	private float arctic$environmentEnd(float value) {
		return Fog.stretch(value);
	}

	@ModifyVariable(method = WRITE, at = @At("HEAD"), argsOnly = true, ordinal = 2)
	private float arctic$renderStart(float value) {
		return Fog.stretch(value);
	}

	@ModifyVariable(method = WRITE, at = @At("HEAD"), argsOnly = true, ordinal = 3)
	private float arctic$renderEnd(float value) {
		return Fog.stretch(value);
	}
	//#else
	@Inject(method = "setupFog", at = @At("RETURN"), cancellable = true)
	private static void arctic$fog(CallbackInfoReturnable<FogParameters> cir) {
		FogParameters fog = cir.getReturnValue();
		float f = Fog.factor();
		if (f != 1f && fog != null) {
			cir.setReturnValue(new FogParameters(Fog.stretch(fog.start(), f), Fog.stretch(fog.end(), f), fog.shape(), fog.red(), fog.green(), fog.blue(), fog.alpha()));
		}
	}
	//#endif
}
//#endif
