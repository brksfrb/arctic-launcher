//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ParticleFilter;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleEngine;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Particle Control: every particle the game adds passes the filter first. */
@Mixin(ParticleEngine.class)
abstract class ParticleEngineMixin {
	@Inject(method = "add(Lnet/minecraft/client/particle/Particle;)V", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$filter(Particle particle, CallbackInfo ci) {
		if (!ParticleFilter.keep(ArcticClient.config())) {
			ci.cancel();
		}
	}
}
//#endif
