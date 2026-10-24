package com.arcticlauncher.mod.mixin;

//#if MC >= 1.14 && MC < 1.21.2
import com.arcticlauncher.client.feature.DynamicFps;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Dynamic FPS: the game's frame rate limit, lowered while the game isn't in use. */
@Mixin(Minecraft.class)
abstract class FramerateLimitMixin {
	@Inject(method = "getFramerateLimit", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$dynamic(CallbackInfoReturnable<Integer> cir) {
		cir.setReturnValue(DynamicFps.limit(cir.getReturnValue()));
	}
}
//#endif
