//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.world.level.lighting.LevelLightEngine;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Fullbright, faster: with Fullbright on and "Skip light calculations" chosen, the client's light
 * engine does no work at all. Everything it would compute is thrown away by the maximum brightness
 * Fullbright draws with anyway, so the CPU time (chunk loads, block changes) is saved.
 * <p>
 * Every call that would queue work is dropped, and the run that would do it returns at once, so
 * nothing piles up. The names changed in 1.20 (onBlockEmissionIncrease and enableLightSources became
 * setLightEnabled and propagateLightSources, runUpdates became runLightUpdates).
 */
@Mixin(LevelLightEngine.class)
abstract class LightEngineMixin {
	/**
	 * The light engine is idle: Fullbright on, skipping chosen. Only the client's own engine, which is
	 * a plain LevelLightEngine: the single-player server's (a subclass) lights the real world and its
	 * save, and must never be stopped.
	 */
	private static boolean arctic$skip(Object engine) {
		ClientConfig config = ArcticClient.config();
		return config != null && config.fullbright && config.fullbrightSkipLight
				&& engine.getClass() == LevelLightEngine.class;
	}

	//#if MC >= 1.20
	@Inject(method = {"checkBlock", "updateSectionStatus", "setLightEnabled", "propagateLightSources", "queueSectionData",
			"retainData"},
			at = @At("HEAD"), cancellable = true, require = 0)
	//#else
	@Inject(method = {"checkBlock", "onBlockEmissionIncrease", "updateSectionStatus", "enableLightSources",
			"queueSectionData", "retainData"},
			at = @At("HEAD"), cancellable = true, require = 0)
	//#endif
	private void arctic$dropWork(CallbackInfo ci) {
		if (arctic$skip(this)) {
			ci.cancel();
		}
	}

	//#if MC >= 1.20
	@Inject(method = "runLightUpdates", at = @At("HEAD"), cancellable = true, require = 0)
	//#else
	@Inject(method = "runUpdates", at = @At("HEAD"), cancellable = true, require = 0)
	//#endif
	private void arctic$noRun(CallbackInfoReturnable<Integer> cir) {
		if (arctic$skip(this)) {
			cir.setReturnValue(0);
		}
	}
}
//#endif
