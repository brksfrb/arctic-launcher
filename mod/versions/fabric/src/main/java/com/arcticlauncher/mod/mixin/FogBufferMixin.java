package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.5
import com.arcticlauncher.client.Fog;
//#if MC >= 1.21.6
import net.minecraft.client.renderer.fog.FogData;
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
 * FogRenderer.setupFog (stretched on its way out); 1.21.6 and newer fill a FogData in FogRenderer.setupFog and the
 * game (and Sodium, which copies the FogData for its own chunk shader) read the distances from it, so the FogData is
 * stretched where it is complete, before anything reads it. Stretching the vanilla buffer write instead would leave
 * Sodium's terrain fog untouched.
 */
@Mixin(FogRenderer.class)
abstract class FogBufferMixin {
	//#if MC >= 1.21.6
	/** Right after setupFog stored the last of the distances (PUTFIELD = 181), before updateBuffer, Sodium or the return read them. */
	@ModifyVariable(method = "setupFog", at = @At(value = "FIELD", target = "Lnet/minecraft/client/renderer/fog/FogData;renderDistanceEnd:F", opcode = 181, shift = At.Shift.AFTER), ordinal = 0)
	private FogData arctic$fog(FogData data) {
		float f = Fog.factor();
		if (f != 1f && data != null) {
			data.environmentalStart = Fog.stretch(data.environmentalStart, f);
			data.environmentalEnd = Fog.stretch(data.environmentalEnd, f);
			data.renderDistanceStart = Fog.stretch(data.renderDistanceStart, f);
			data.renderDistanceEnd = Fog.stretch(data.renderDistanceEnd, f);
			data.skyEnd = Fog.stretch(data.skyEnd, f);
			data.cloudEnd = Fog.stretch(data.cloudEnd, f);
		}
		return data;
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
