package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.world.World;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fullbright, faster: with "Skip light calculations" chosen, the client's world does no light
 * recalculation (Fullbright draws at maximum brightness anyway). Only the client's own world:
 * the single-player server's world lights the real save and is never touched.
 */
@Mixin(World.class)
abstract class LightWorkMixin {
	@Shadow
	@Final
	public boolean isClient;

	@Inject(method = "calculateLightAtPos", at = @At("HEAD"), cancellable = true)
	private void arctic$skipLight(CallbackInfoReturnable<Boolean> cir) {
		ClientConfig config = ArcticClient.config();
		if (isClient && config != null && config.fullbright && config.fullbrightSkipLight) {
			cir.setReturnValue(false);
		}
	}
}
