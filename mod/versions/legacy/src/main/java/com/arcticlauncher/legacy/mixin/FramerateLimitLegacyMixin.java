package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.DynamicFps;
import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Dynamic FPS: the game's frame rate limit, lowered while the game isn't in use. */
@Mixin(MinecraftClient.class)
abstract class FramerateLimitLegacyMixin {
	@Inject(method = "getMaxFramerate", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$dynamic(CallbackInfoReturnable<Integer> cir) {
		cir.setReturnValue(DynamicFps.limit(cir.getReturnValue()));
	}
}
