package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.Entity;
import org.jspecify.annotations.Nullable;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Other players are ticked together on several threads (see
 * {@link ParallelTicks}): set aside during the entity tick, ticked at its
 * end; their particles, sounds and level events wait for the render thread.
 */
@Mixin(ClientLevel.class)
abstract class ClientLevelParallelTicksMixin {
	@Shadow
	private void doAddParticle(ParticleOptions particle, boolean overrideLimiter, boolean alwaysShowParticles, double x, double y, double z,
			double xd, double yd, double zd) {}

	private ClientLevel polonium$self() {
		return (ClientLevel) (Object) this;
	}

	@Inject(method = "tickEntities", at = @At("HEAD"))
	private void polonium$begin(CallbackInfo ci) {
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.TICK_ENTITIES);
		ParallelTicks.begin();
	}

	@Inject(method = "tickEntities", at = @At("TAIL"))
	private void polonium$tickSetAside(CallbackInfo ci) {
		com.arcticlauncher.polonium.Timeline.start(com.arcticlauncher.polonium.Timeline.Step.PARALLEL_TICKS);
		ParallelTicks.end(polonium$self());
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.PARALLEL_TICKS);
		com.arcticlauncher.polonium.Timeline.end(com.arcticlauncher.polonium.Timeline.Step.TICK_ENTITIES);
	}

	/**
	 * Set aside before tickNonPassenger is even called, so hooks at its head
	 * run once, in the parallel tick. Entity culling's does: it marks the
	 * entity out of view (until it's rendered) and ticks it lightly if it
	 * still is; run once before setting aside and again in the parallel
	 * tick, it saw its own mark and gave every visible player the light tick
	 * (no walk animation, no body turn: legs still or racing, heads spinning).
	 */
	@WrapOperation(method = "lambda$tickEntities$0",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/multiplayer/ClientLevel;guardEntityTick(Ljava/util/function/Consumer;Lnet/minecraft/world/entity/Entity;)V"))
	private void polonium$setAside(ClientLevel level, java.util.function.Consumer<Entity> tick, Entity entity, Operation<Void> guarded) {
		if (!ParallelTicks.collect(entity)) {
			guarded.call(level, tick, entity);
		}
	}

	@Inject(method = "doAddParticle", at = @At("HEAD"), cancellable = true)
	private void polonium$particleLater(ParticleOptions particle, boolean overrideLimiter, boolean alwaysShowParticles, double x, double y,
			double z, double xd, double yd, double zd, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> doAddParticle(particle, overrideLimiter, alwaysShowParticles, x, y, z, xd, yd, zd));
			ci.cancel();
		}
	}

	@Inject(method = "playSeededSound(Lnet/minecraft/world/entity/Entity;DDDLnet/minecraft/core/Holder;Lnet/minecraft/sounds/SoundSource;FFJ)V",
			at = @At("HEAD"), cancellable = true)
	private void polonium$soundLater(@Nullable Entity except, double x, double y, double z, Holder<SoundEvent> sound, SoundSource source,
			float volume, float pitch, long seed, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().playSeededSound(except, x, y, z, sound, source, volume, pitch, seed));
			ci.cancel();
		}
	}

	@Inject(method = "playSeededSound(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/entity/Entity;Lnet/minecraft/core/Holder;Lnet/minecraft/sounds/SoundSource;FFJ)V",
			at = @At("HEAD"), cancellable = true)
	private void polonium$entitySoundLater(@Nullable Entity except, Entity sourceEntity, Holder<SoundEvent> sound, SoundSource source,
			float volume, float pitch, long seed, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().playSeededSound(except, sourceEntity, sound, source, volume, pitch, seed));
			ci.cancel();
		}
	}

	@Inject(method = "playLocalSound(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundSource;FF)V",
			at = @At("HEAD"), cancellable = true)
	private void polonium$localSoundLater(Entity sourceEntity, SoundEvent sound, SoundSource source, float volume, float pitch,
			CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().playLocalSound(sourceEntity, sound, source, volume, pitch));
			ci.cancel();
		}
	}

	@Inject(method = "playLocalSound(DDDLnet/minecraft/sounds/SoundEvent;Lnet/minecraft/sounds/SoundSource;FFZ)V",
			at = @At("HEAD"), cancellable = true)
	private void polonium$positionedSoundLater(double x, double y, double z, SoundEvent sound, SoundSource source, float volume, float pitch,
			boolean distanceDelay, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().playLocalSound(x, y, z, sound, source, volume, pitch, distanceDelay));
			ci.cancel();
		}
	}

	@Inject(method = "playPlayerSound", at = @At("HEAD"), cancellable = true)
	private void polonium$playerSoundLater(SoundEvent sound, SoundSource source, float volume, float pitch, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().playPlayerSound(sound, source, volume, pitch));
			ci.cancel();
		}
	}

	@Inject(method = "levelEvent", at = @At("HEAD"), cancellable = true)
	private void polonium$levelEventLater(@Nullable Entity source, int type, BlockPos pos, int data, CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			ParallelTicks.defer(() -> polonium$self().levelEvent(source, type, pos, data));
			ci.cancel();
		}
	}
}
