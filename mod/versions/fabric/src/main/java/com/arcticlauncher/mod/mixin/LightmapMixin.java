//#if MC >= 1.20
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Features;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
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
}
//#endif
