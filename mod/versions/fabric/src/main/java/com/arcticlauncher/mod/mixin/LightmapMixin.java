package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Features;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Minecraft;
//#if MC >= 1.19
import net.minecraft.client.OptionInstance;
//#else
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif
//#if MC >= 26.1
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
//#else
import net.minecraft.client.renderer.LightTexture;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Fullbright: the lightmap reads a very high gamma. The brightness option
 * itself is untouched, so options.txt keeps the player's own value.
 * 26.1 renamed LightTexture/updateLightTexture to this extractor/extract.
 */
//#if MC >= 26.1
@Mixin(LightmapRenderStateExtractor.class)
//#else
@Mixin(LightTexture.class)
//#endif
abstract class LightmapMixin {
	//#if MC >= 1.19
	@WrapOperation(
			//#if MC >= 26.1
			method = "extract",
			//#else
			method = "updateLightTexture",
			//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/OptionInstance;get()Ljava/lang/Object;"))
	private Object arctic$gamma(OptionInstance<?> option, Operation<Object> original) {
		if (ArcticClient.features().fullbright() && option == Minecraft.getInstance().options.gamma()) {
			return Features.FULLBRIGHT_GAMMA;
		}
		return original.call(option);
	}
	//#else
	/** Before 1.19 brightness was a plain number on the options. */
	@Redirect(method = "updateLightTexture",
			at = @At(value = "FIELD", target = "Lnet/minecraft/client/Options;gamma:D"))
	private double arctic$gamma(net.minecraft.client.Options options) {
		return ArcticClient.features().fullbright() ? Features.FULLBRIGHT_GAMMA : options.gamma;
	}
	//#endif
}
