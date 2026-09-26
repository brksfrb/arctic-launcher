package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Features;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Fullbright: the lightmap reads a very high gamma. The brightness option
 * itself is untouched, so options.txt keeps the player's own value.
 * Listed in the mixin config only where this target exists (26.3).
 */
@Mixin(LightmapRenderStateExtractor.class)
abstract class LightmapMixin {
	@WrapOperation(method = "extract", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/OptionInstance;get()Ljava/lang/Object;"))
	private Object arctic$gamma(OptionInstance<?> option, Operation<Object> original) {
		if (ArcticClient.features().fullbright() && option == Minecraft.getInstance().options.gamma()) {
			return Features.FULLBRIGHT_GAMMA;
		}
		return original.call(option);
	}
}
//#endif
