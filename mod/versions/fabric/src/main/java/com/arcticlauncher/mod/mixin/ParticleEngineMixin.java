//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ParticleFilter;
import com.arcticlauncher.client.feature.ParticleKinds;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleEngine;
import net.minecraft.core.particles.ParticleOptions;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Particle Control: every particle the game adds passes the filter first. */
@Mixin(ParticleEngine.class)
abstract class ParticleEngineMixin {
	@Inject(method = "add(Lnet/minecraft/client/particle/Particle;)V", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$filter(Particle particle, CallbackInfo ci) {
		if (!ParticleFilter.keep(ArcticClient.config()) || arctic$isBlockBit(particle)) {
			ci.cancel();
		}
	}

	/** Particle Control, per kind: a hidden kind is never created. */
	@Inject(method = "createParticle", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$kind(ParticleOptions options, double x, double y, double z, double vx, double vy, double vz,
			CallbackInfoReturnable<Particle> cir) {
		com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
		if (c.particleControl && c.particleOff != 0 && ParticleKinds.hidden(c, com.arcticlauncher.mod.Compat.particlePath(options))) {
			cir.setReturnValue(null);
		}
	}

	/** Block breaking and stepping dust: made straight from the block, not through createParticle. */
	private static boolean arctic$isBlockBit(Particle particle) {
		return particle instanceof net.minecraft.client.particle.TerrainParticle && ParticleKinds.off(ArcticClient.config(), ParticleKinds.BLOCK);
	}
}
//#endif
