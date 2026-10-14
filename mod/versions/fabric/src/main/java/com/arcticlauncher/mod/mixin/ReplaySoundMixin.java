package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.SoundCapture;
import java.util.Map;
import net.minecraft.client.Options;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.SoundEngine;
import net.minecraft.sounds.SoundSource;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Observe resolved, active channels: no changes to the game's sound playback. */
@Mixin(SoundEngine.class)
abstract class ReplaySoundMixin {
	@Shadow @Final private Map<SoundInstance, ?> instanceToChannel;
	@Shadow @Final private Options options;
	@Unique private final SoundCapture arctic$audio = new SoundCapture();
	@Unique private final Map<SoundInstance, float[]> arctic$levels = new java.util.IdentityHashMap<SoundInstance, float[]>();
	// Re-reading SoundInstance.getPitch/getVolume can re-roll a resource pack's random values.
	// Observe the values the engine actually sends to its channels instead.
	@Inject(method = "calculateVolume(Lnet/minecraft/client/resources/sounds/SoundInstance;)F", at = @At("RETURN"))
	private void arctic$volume(SoundInstance sound, CallbackInfoReturnable<Float> cir) {
		arctic$levels.computeIfAbsent(sound, s -> new float[] {1, 1})[0] = cir.getReturnValue();
	}
	@Inject(method = "calculatePitch", at = @At("RETURN"))
	private void arctic$pitch(SoundInstance sound, CallbackInfoReturnable<Float> cir) {
		arctic$levels.computeIfAbsent(sound, s -> new float[] {1, 1})[1] = cir.getReturnValue();
	}
	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$capture(boolean paused, CallbackInfo ci) {
		arctic$levels.keySet().retainAll(instanceToChannel.keySet());
		if (!arctic$audio.begin()) return;
		for (SoundInstance s : instanceToChannel.keySet()) {
			if (s.getSound() == null) continue;
			float[] levels = arctic$levels.get(s);
			if (levels == null) continue;
			float volume = levels[0] * options.getSoundSourceVolume(SoundSource.MASTER);
			arctic$audio.sound(s, s.getSound().getPath().toString(), volume, levels[1],
					s.isLooping() && s.getDelay() == 0, s.getX(), s.getY(), s.getZ(), s.isRelative(),
					s.getAttenuation() == SoundInstance.Attenuation.NONE ? 0 : Math.max(1, levels[0]) * s.getSound().getAttenuationDistance());
		}
		arctic$audio.end();
	}
}
