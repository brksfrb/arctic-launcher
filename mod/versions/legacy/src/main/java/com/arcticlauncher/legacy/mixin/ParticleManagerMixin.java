package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ParticleFilter;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleManager;
import net.minecraft.block.BlockState;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import com.arcticlauncher.client.feature.ParticleKinds;
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

	/** Particle Control, per kind: a hidden kind is never created. */
	@Inject(method = "addParticle(IDDDDDD[I)Lnet/minecraft/client/particle/Particle;", at = @At("HEAD"), cancellable = true)
	private void arctic$kind(int id, double x, double y, double z, double vx, double vy, double vz, int[] args,
			CallbackInfoReturnable<Particle> cir) {
		if (ParticleKinds.hiddenLegacy(ArcticClient.config(), id)) {
			cir.setReturnValue(null);
		}
	}

	@Inject(method = "addBlockBreakParticles", at = @At("HEAD"), cancellable = true)
	private void arctic$noBreak(BlockPos pos, BlockState state, CallbackInfo ci) {
		if (ParticleKinds.off(ArcticClient.config(), ParticleKinds.BLOCK)) {
			ci.cancel();
		}
	}

	@Inject(method = "addBlockBreakingParticles", at = @At("HEAD"), cancellable = true)
	private void arctic$noBreaking(BlockPos pos, Direction side, CallbackInfo ci) {
		if (ParticleKinds.off(ArcticClient.config(), ParticleKinds.BLOCK)) {
			ci.cancel();
		}
	}
}
