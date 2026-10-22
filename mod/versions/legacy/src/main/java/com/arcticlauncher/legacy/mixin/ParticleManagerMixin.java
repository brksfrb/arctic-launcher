package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ParticleFilter;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Particle Control: every particle the game adds passes the filter first. */
@Mixin(ParticleManager.class)
abstract class ParticleManagerMixin {
	//#if MC >= 1.9
	private static final String ADD = "method_12256";
	//#else
	private static final String ADD = "addParticle(Lnet/minecraft/client/particle/Particle;)V";
	//#endif

	@Inject(method = ADD, at = @At("HEAD"), cancellable = true)
	private void arctic$filter(Particle particle, CallbackInfo ci) {
		if (!ParticleFilter.keep(ArcticClient.config())) {
			ci.cancel();
		}
	}
}
